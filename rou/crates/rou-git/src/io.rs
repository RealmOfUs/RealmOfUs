//! Bounded filesystem IO, symlink rejection, atomic writes and disposable host caches.
use crate::{Context, Result, ensure};
use rou_core::{documents::Descriptor, json, limits, paths};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs::{self, File, Metadata},
    io::{Read, Write},
    path::{Path, PathBuf},
};

/// Reject an existing symlink at this path or any ancestor; tolerate missing components.
pub fn reject_symlinks(path: &Path) -> Result<()> {
    for component in path.ancestors() {
        match fs::symlink_metadata(component) {
            Ok(metadata) => ensure!(
                "unsafe_path",
                !metadata.file_type().is_symlink(),
                "symlink forbidden: {}",
                component.display()
            ),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
    }
    Ok(())
}

/// Validate a repository-relative byte path and reject symlinks before returning its destination.
pub fn destination(root: &Path, relative: &[u8]) -> Result<PathBuf> {
    paths::safe_bytes(relative)?;
    let path = root.join(crate::repository::os_path(relative)?);
    reject_symlinks(&path)?;
    Ok(path)
}

/// Resolve a safe path beneath `.rou/`, rejecting symlinks.
pub fn local(root: &Path, relative: &str) -> Result<PathBuf> {
    paths::safe(relative)?;
    let path = root.join(paths::LOCAL).join(relative);
    reject_symlinks(&path)?;
    Ok(path)
}

/// Read a regular file within `max` bytes, rejecting symlinks and growth beyond the budget.
pub fn read(path: &Path, max: usize) -> Result<Vec<u8>> {
    reject_symlinks(path)?;
    let file = File::open(path).with_context(|| format!("missing file: {}", path.display()))?;
    let metadata = file.metadata()?;
    ensure!(
        "byte_limit",
        metadata.is_file() && metadata.len() <= max as u64,
        "file byte budget exceeded: {}",
        path.display()
    );
    let mut data = Vec::with_capacity(metadata.len() as usize);
    file.take(max as u64 + 1).read_to_end(&mut data)?;
    ensure!(
        "byte_limit",
        data.len() <= max,
        "file grew beyond byte budget: {}",
        path.display()
    );
    Ok(data)
}

/// Write and sync a sibling temporary file, then atomically replace the destination.
/// Create parent directories and reject symlinks before and after preparation.
pub fn atomic_write(path: &Path, data: &[u8]) -> Result<()> {
    reject_symlinks(path)?;
    let parent = path.parent().context("output has no parent")?;
    fs::create_dir_all(parent)?;
    let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
    temporary.write_all(data)?;
    temporary.as_file().sync_all()?;
    reject_symlinks(path)?;
    temporary.persist(path).map_err(|e| e.error)?;
    Ok(())
}

/// Write a versioned JSON document using the core pretty encoder and its document budget.
pub fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    atomic_write(path, &json::pretty(value)?)
}

// Local hints have a separate, generous budget and never prevent an operation.
const CACHE_BYTES: usize = 128 * 1024 * 1024;
// Fixed-size keys and validated descriptors keep this well below CACHE_BYTES.
const HASH_CACHE_ENTRIES: usize = 100_000;

/// Read a disposable host cache, treating missing, oversized or corrupt data as empty.
pub fn read_cache<T: serde::de::DeserializeOwned + Default>(path: &Path) -> T {
    read(path, CACHE_BYTES)
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default()
}

/// Best-effort atomic cache persistence, independent of versioned JSON limits.
pub fn write_cache<T: Serialize>(path: &Path, value: &T) {
    if let Ok(bytes) = serde_json::to_vec(value) {
        if bytes.len() <= CACHE_BYTES {
            let _ = atomic_write(path, &bytes);
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
/// File identity and timestamp tuple used to invalidate disposable local caches.
pub struct Stamp {
    size: u64,
    inode: u64,
    modified: i64,
    modified_ns: i64,
    changed: i64,
    changed_ns: i64,
}
impl Stamp {
    #[cfg(unix)]
    /// Capture file identity, size and modification/change timestamps for cache invalidation.
    pub fn of(metadata: &Metadata) -> Self {
        use std::os::unix::fs::MetadataExt;
        Self {
            size: metadata.len(),
            inode: metadata.ino(),
            modified: metadata.mtime(),
            modified_ns: metadata.mtime_nsec(),
            changed: metadata.ctime(),
            changed_ns: metadata.ctime_nsec(),
        }
    }
    #[cfg(not(unix))]
    /// Capture file identity, size and modification/change timestamps for cache invalidation.
    pub fn of(metadata: &Metadata) -> Self {
        let modified = metadata
            .modified()
            .ok()
            .and_then(|m| m.duration_since(std::time::UNIX_EPOCH).ok())
            .unwrap_or_default();
        Self {
            size: metadata.len(),
            inode: 0,
            modified: modified.as_secs() as i64,
            modified_ns: modified.subsec_nanos() as i64,
            changed: 0,
            changed_ns: 0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Cached {
    stamp: Stamp,
    descriptor: Descriptor,
    last_used: u64,
}

/// Source descriptors keyed by native absolute path and checked file stamps.
pub struct HashCache {
    path: PathBuf,
    entries: BTreeMap<String, Cached>,
    transient: BTreeMap<String, Cached>,
    clock: u64,
    dirty: bool,
}
impl HashCache {
    /// Load the disposable source descriptor cache; corrupt or missing data starts empty.
    pub fn load(root: &Path) -> Result<Self> {
        let path = local(root, "cache/source-hashes.json")?;
        // Caches are disposable hints; corrupted caches are rebuilt from actual files.
        let mut entries: BTreeMap<String, Cached> = read_cache(&path);
        entries.retain(|key, entry| key.len() == 64 && entry.descriptor.validate_shape().is_ok());
        trim_hashes(&mut entries, HASH_CACHE_ENTRIES);
        let clock = entries
            .values()
            .map(|entry| entry.last_used)
            .max()
            .unwrap_or(0);
        Ok(Self {
            path,
            entries,
            transient: BTreeMap::new(),
            clock,
            dirty: false,
        })
    }
    /// Return a cached descriptor only when file identity and timestamps still match.
    /// Otherwise stream SHA-256 and reject files that exceed the source budget or change during hashing.
    pub fn descriptor(&mut self, path: &Path) -> Result<Descriptor> {
        self.describe(path, true)
    }
    /// Cache a frozen file or LFS object only for this operation, never on disk.
    pub fn transient_descriptor(&mut self, path: &Path) -> Result<Descriptor> {
        self.describe(path, false)
    }
    fn describe(&mut self, path: &Path, persist: bool) -> Result<Descriptor> {
        reject_symlinks(path)?;
        let mut file =
            File::open(path).with_context(|| format!("missing file: {}", path.display()))?;
        let metadata = file.metadata()?;
        ensure!(
            "byte_limit",
            metadata.is_file() && metadata.len() <= limits::FILE_BYTES,
            "source byte budget exceeded"
        );
        let stamp = Stamp::of(&metadata);
        let key = json::digest(path.as_os_str().as_encoded_bytes());
        self.clock = self.clock.saturating_add(1);
        let entries = if persist {
            &mut self.entries
        } else {
            &mut self.transient
        };
        if let Some(entry) = entries.get_mut(&key) {
            if entry.stamp == stamp && entry.descriptor.validate_shape().is_ok() {
                entry.last_used = self.clock;
                // Persist recency along with content changes, not on read-only hits.
                return Ok(entry.descriptor.clone());
            }
        }
        let mut hash = Sha256::new();
        let mut bytes = 0_u64;
        let mut buffer = [0; 65536];
        loop {
            let count = file.read(&mut buffer)?;
            if count == 0 {
                break;
            }
            bytes += count as u64;
            ensure!(
                "byte_limit",
                bytes <= limits::FILE_BYTES,
                "source grew beyond byte budget"
            );
            hash.update(&buffer[..count]);
        }
        ensure!(
            "file_changed",
            stamp == Stamp::of(&file.metadata()?),
            "source changed while hashing: {}",
            path.display()
        );
        let descriptor = Descriptor {
            sha256: format!("{:x}", hash.finalize()),
            bytes,
        };
        if entries.len() >= HASH_CACHE_ENTRIES && !entries.contains_key(&key) {
            trim_hashes(entries, HASH_CACHE_ENTRIES * 7 / 8);
        }
        entries.insert(
            key,
            Cached {
                stamp,
                descriptor: descriptor.clone(),
                last_used: self.clock,
            },
        );
        self.dirty |= persist;
        Ok(descriptor)
    }
    /// Best-effort persist changed entries; cache write failure never rejects source content.
    pub fn flush(&mut self) -> Result<()> {
        if self.dirty {
            write_cache(&self.path, &self.entries);
            self.dirty = false;
        }
        Ok(())
    }
}

fn trim_hashes(entries: &mut BTreeMap<String, Cached>, keep: usize) {
    if entries.len() <= keep {
        return;
    }
    let mut age: Vec<_> = entries
        .iter()
        .map(|(key, value)| (value.last_used, key.clone()))
        .collect();
    age.sort_unstable();
    for (_, key) in age.into_iter().take(entries.len() - keep) {
        entries.remove(&key);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_cache_invalidates_rewrites_and_replacements_with_same_size_and_mtime() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap();
        let source = root.join("source.blend");
        fs::write(&source, b"first").unwrap();
        let modified = source.metadata().unwrap().modified().unwrap();
        let mut cache = HashCache::load(&root).unwrap();
        let first = cache.descriptor(&source).unwrap();
        cache.flush().unwrap();
        let mut cache = HashCache::load(&root).unwrap();
        assert_eq!(cache.descriptor(&source).unwrap(), first);
        std::thread::sleep(std::time::Duration::from_millis(2));
        fs::write(&source, b"other").unwrap();
        File::options()
            .write(true)
            .open(&source)
            .unwrap()
            .set_times(fs::FileTimes::new().set_modified(modified))
            .unwrap();
        assert_ne!(cache.descriptor(&source).unwrap(), first);
        atomic_write(&source, b"third").unwrap();
        File::options()
            .write(true)
            .open(&source)
            .unwrap()
            .set_times(fs::FileTimes::new().set_modified(modified))
            .unwrap();
        assert_eq!(cache.descriptor(&source).unwrap(), Descriptor::of(b"third"));
    }

    #[test]
    fn large_and_corrupt_local_caches_are_independent_of_document_budgets() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap();
        let source = root.join("source.blend");
        fs::write(&source, b"data").unwrap();
        let cached = Cached {
            stamp: Stamp::of(&source.metadata().unwrap()),
            descriptor: Descriptor::of(b"data"),
            last_used: 0,
        };
        let mut cache = HashCache::load(&root).unwrap();
        cache.entries = (0..25_000)
            .map(|i| (format!("{i:064x}"), cached.clone()))
            .collect();
        cache.dirty = true;
        cache.flush().unwrap();
        assert!(cache.path.metadata().unwrap().len() > limits::JSON_BYTES as u64);
        assert_eq!(HashCache::load(&root).unwrap().entries.len(), 25_000);
        fs::write(&cache.path, b"corrupt cache").unwrap();
        assert_eq!(
            HashCache::load(&root).unwrap().descriptor(&source).unwrap(),
            Descriptor::of(b"data")
        );
        let lineage = root.join("lineage.json");
        let entries: BTreeMap<String, Option<String>> = (0..50_000)
            .map(|i| (format!("{i:040x}"), Some(format!("{i:064x}"))))
            .collect();
        write_cache(&lineage, &entries);
        assert!(lineage.metadata().unwrap().len() > limits::JSON_BYTES as u64);
        assert_eq!(
            read_cache::<BTreeMap<String, Option<String>>>(&lineage),
            entries
        );
        // A failed cache write remains an optimization failure only.
        fs::remove_file(&cache.path).unwrap();
        fs::create_dir(&cache.path).unwrap();
        cache.dirty = true;
        cache.flush().unwrap();
    }

    #[test]
    fn transient_hashes_are_not_persisted_and_eviction_keeps_recent_sources() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap();
        let mut cache = HashCache::load(&root).unwrap();
        for i in 0..8 {
            let path = root.join(format!("source-{i}.blend"));
            fs::write(&path, [i]).unwrap();
            cache.descriptor(&path).unwrap();
        }
        let recent = root.join("source-0.blend");
        cache.descriptor(&recent).unwrap();
        for i in 0..20 {
            let run = root.join(format!("run-{i}"));
            fs::write(&run, [i]).unwrap();
            cache.transient_descriptor(&run).unwrap();
            fs::remove_file(run).unwrap();
        }
        trim_hashes(&mut cache.entries, 3);
        cache.flush().unwrap();
        let loaded = HashCache::load(&root).unwrap();
        assert_eq!(loaded.entries.len(), 3);
        assert!(loaded.transient.is_empty());
        assert!(
            loaded
                .entries
                .contains_key(&json::digest(recent.as_os_str().as_encoded_bytes()))
        );
        let fresh = root.join("fresh.blend");
        fs::write(&fresh, b"new source").unwrap();
        cache.descriptor(&fresh).unwrap();
        cache.flush().unwrap();
        assert_eq!(HashCache::load(&root).unwrap().entries.len(), 4);
    }
}
