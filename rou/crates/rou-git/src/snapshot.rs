//! Capture worktree, index or immutable commit metadata and validate realm lineage.
use crate::{
    Context, Result, ensure,
    io::{self, HashCache},
    repository::{Batch, Repository, text},
};
use bstr::{BString, ByteSlice};
use rou_core::{
    documents::{Build, Descriptor, Entity, Model, Realm},
    json, lfs, limits, paths,
    world::{self, World},
};
use std::{
    cell::RefCell,
    collections::{BTreeMap, BTreeSet},
    fs::File,
    io::Read,
    path::PathBuf,
};

#[derive(Debug, Clone, PartialEq, Eq)]
/// The input authority for a repository snapshot.
pub enum Mode {
    /// Live worktree inputs, with Git fallback only for sparse-absent files.
    Worktree,
    /// Staged Git entries, independent of unstaged worktree edits.
    Index,
    /// An immutable commit identified by its full OID.
    Commit(String),
}
impl Mode {
    /// Return the immutable commit OID only for commit mode.
    pub fn commit(&self) -> Option<&str> {
        if let Self::Commit(oid) = self {
            Some(oid)
        } else {
            None
        }
    }
    /// Return WORKTREE, INDEX or the full commit OID for diagnostics.
    pub fn label(&self) -> &str {
        match self {
            Self::Worktree => "WORKTREE",
            Self::Index => "INDEX",
            Self::Commit(oid) => oid,
        }
    }
}

#[derive(Debug, Clone)]
/// An immutable Git blob reference captured from the tree or index.
pub struct Entry {
    /// Input snapshot label, or Git file mode for an index entry.
    pub mode: String,
    /// Full immutable Git object ID.
    pub oid: String,
    /// Git index stage: zero normally, one through three for conflicts.
    pub stage: u8,
}

/// A scoped set of paths, blob reads, source hashes and LFS configuration.
/// Create a new instance after changing refs, the index, worktree metadata or Git configuration.
pub struct Snapshot {
    /// Repository owning this snapshot.
    pub repo: Repository,
    /// Input snapshot label, or Git file mode for an index entry.
    pub mode: Mode,
    /// Stage-zero or committed regular entries, keyed by raw Git path bytes.
    pub entries: BTreeMap<BString, Entry>,
    /// Nonzero index stages grouped by raw Git path bytes.
    pub conflicts: BTreeMap<BString, Vec<Entry>>,
    /// Paths marked skip-worktree when this snapshot was captured.
    pub skipped: BTreeSet<BString>,
    /// Paths visible to this snapshot, including live nonignored untracked inputs in worktree mode.
    pub paths: BTreeSet<BString>,
    /// Exact logical index bytes used to parse this snapshot; empty for commit mode.
    pub(crate) index: Vec<u8>,
    batch: RefCell<Batch>,
    cache: RefCell<HashCache>,
    raw: RefCell<BTreeMap<BString, Vec<u8>>>,
    pub(crate) media: RefCell<Option<PathBuf>>,
}
impl Snapshot {
    /// Capture path names, index stages and sparse flags; start one batched blob reader.
    /// Worktree reads remain live and require a caller baseline check before publishing outputs.
    pub fn new(repo: &Repository, mode: Mode) -> Result<Self> {
        let mut entries = BTreeMap::new();
        let mut conflicts: BTreeMap<BString, Vec<Entry>> = BTreeMap::new();
        let mut skipped = BTreeSet::new();
        let mut index = Vec::new();
        if let Mode::Commit(ref oid) = mode {
            ensure!(
                "invalid_revision",
                (oid.len() == 40 || oid.len() == 64) && oid.bytes().all(|c| c.is_ascii_hexdigit()),
                "expected full Git commit OID"
            );
            repo.run(["cat-file", "-e", &format!("{oid}^{{commit}}")])?;
            for item in repo
                .run(["ls-tree", "-r", "-z", oid])?
                .split(|c| *c == 0)
                .filter(|v| !v.is_empty())
            {
                let (header, path) = item
                    .split_once_str(b"\t")
                    .context("invalid Git tree record")?;
                let header: Vec<_> = std::str::from_utf8(header)?.split_whitespace().collect();
                ensure!(
                    "invalid_git_response",
                    header.len() == 3,
                    "invalid tree header"
                );
                entries.insert(
                    BString::from(path),
                    Entry {
                        mode: header[0].into(),
                        oid: header[2].into(),
                        stage: 0,
                    },
                );
            }
        } else {
            index = repo.run(["ls-files", "--stage", "-z"])?;
            for item in index.split(|c| *c == 0).filter(|v| !v.is_empty()) {
                let (header, path) = item.split_once_str(b"\t").context("invalid index record")?;
                let header: Vec<_> = std::str::from_utf8(header)?.split_whitespace().collect();
                ensure!(
                    "invalid_git_response",
                    header.len() == 3,
                    "invalid index header"
                );
                let entry = Entry {
                    mode: header[0].into(),
                    oid: header[1].into(),
                    stage: header[2].parse()?,
                };
                if entry.stage == 0 {
                    entries.insert(BString::from(path), entry);
                } else {
                    conflicts.entry(path.into()).or_default().push(entry);
                }
            }
            for item in repo.run(["ls-files", "-v", "-z"])?.split(|c| *c == 0) {
                if item.len() > 2 && matches!(item[0], b'S' | b's') {
                    skipped.insert(BString::from(&item[2..]));
                }
            }
        }
        let mut all_paths: BTreeSet<_> = entries.keys().chain(conflicts.keys()).cloned().collect();
        if mode == Mode::Worktree {
            all_paths.extend(
                repo.run(["ls-files", "--others", "--exclude-standard", "-z"])?
                    .split(|c| *c == 0)
                    .filter(|p| !p.is_empty())
                    .map(BString::from),
            );
            all_paths.retain(|path| {
                skipped.contains(path)
                    || conflicts.contains_key(path)
                    || crate::repository::os_path(path)
                        .is_ok_and(|p| repo.root.join(p).symlink_metadata().is_ok())
            });
        }
        Ok(Self {
            repo: repo.clone(),
            mode,
            entries,
            conflicts,
            skipped,
            paths: all_paths,
            index,
            batch: RefCell::new(repo.batch()?),
            cache: RefCell::new(HashCache::load(&repo.root)?),
            raw: RefCell::new(BTreeMap::new()),
            media: RefCell::new(None),
        })
    }
    /// Read a captured regular-file Git entry within a byte budget; reject symlinks/submodules.
    pub fn read_entry(&self, entry: &Entry, max: usize) -> Result<Vec<u8>> {
        ensure!(
            "unsafe_path",
            matches!(entry.mode.as_str(), "100644" | "100755"),
            "symlink/submodule is not a realm input"
        );
        self.batch
            .borrow_mut()
            .read(&entry.oid, max)?
            .context("missing Git blob")
    }
    /// Read bounded metadata from the selected mode, caching small bytes in this snapshot.
    /// Use Git for sparse-absent files, but reject ordinary deletions and unresolved conflicts.
    pub fn read(&self, path: &[u8], max: usize) -> Result<Vec<u8>> {
        paths::safe_bytes(path)?;
        ensure!(
            "unresolved_conflict",
            !self.conflicts.contains_key(path),
            "unresolved input conflict: {}",
            path.as_bstr()
        );
        if let Some(data) = self.raw.borrow().get(path) {
            ensure!(
                "byte_limit",
                data.len() <= max,
                "cached file exceeds byte budget"
            );
            return Ok(data.clone());
        }
        let file = io::destination(&self.repo.root, path)?;
        let bytes = if self.mode == Mode::Worktree && file.exists() {
            io::read(&file, max)?
        } else {
            ensure!(
                "deleted_file",
                self.mode != Mode::Worktree || self.skipped.contains(path),
                "file deleted, not sparse: {}",
                path.as_bstr()
            );
            self.read_entry(
                self.entries
                    .get(path)
                    .with_context(|| format!("missing project file: {}", path.as_bstr()))?,
                max,
            )?
        };
        // Only lightweight metadata is cached; source payloads are streamed separately.
        if bytes.len() <= limits::JSON_BYTES {
            self.raw.borrow_mut().insert(path.into(), bytes.clone());
        }
        Ok(bytes)
    }
    /// Describe a source pointer or worktree file; committed sources must fit the LFS pointer budget.
    pub fn descriptor(&self, path: &str) -> Result<Descriptor> {
        paths::safe(path)?;
        ensure!(
            "unresolved_conflict",
            !self.conflicts.contains_key(path.as_bytes()),
            "unresolved input conflict: {path}"
        );
        let file = io::destination(&self.repo.root, path.as_bytes())?;
        if self.mode == Mode::Worktree && file.exists() {
            return self.working_descriptor(path);
        }
        let bytes = self
            .read(path.as_bytes(), limits::POINTER_BYTES)
            .with_context(|| format!("source must be tracked by Git LFS: {path}"))?;
        lfs::descriptor(&bytes).map_err(Into::into)
    }
    /// Describe a present worktree pointer or stream the actual file bytes.
    pub fn working_descriptor(&self, path: &str) -> Result<Descriptor> {
        let file = self.working_path(path)?;
        let mut prefix = Vec::new();
        File::open(&file)?
            .take(limits::POINTER_BYTES as u64 + 1)
            .read_to_end(&mut prefix)?;
        if let Some(pointer) = lfs::pointer(&prefix)? {
            return Ok(pointer);
        }
        self.cache.borrow_mut().descriptor(&file)
    }

    /// Load all author records and input descriptors and validate the world.
    /// Optionally enforce immutable root rules against complete Git ancestry.
    pub fn world(&self, check_lineage: bool) -> Result<World> {
        let realm: Realm = json::parse(&self.read(paths::REALM.as_bytes(), limits::JSON_BYTES)?)?;
        realm.validate()?;
        let mut world = World {
            realm,
            models: BTreeMap::new(),
            entities: BTreeMap::new(),
            inputs: BTreeMap::new(),
        };
        for path in &self.paths {
            if path.starts_with(paths::MODELS.as_bytes()) && path.ends_with(b".json") {
                let model: Model = json::parse(&self.read(path, limits::JSON_BYTES)?)?;
                ensure!(
                    "duplicate_id",
                    !world.models.contains_key(&model.id),
                    "duplicate model ID: {}",
                    model.id
                );
                world::record_id(std::str::from_utf8(path)?, paths::MODELS, &model.id)?;
                let inputs = model
                    .input_paths()
                    .into_iter()
                    .map(|p| self.descriptor(&p).map(|d| (p, d)))
                    .collect::<Result<_>>()?;
                world.inputs.insert(model.id.clone(), inputs);
                world.models.insert(model.id.clone(), model);
            } else if path.starts_with(paths::ENTITIES.as_bytes()) && path.ends_with(b".json") {
                let entity: Entity = json::parse(&self.read(path, limits::JSON_BYTES)?)?;
                ensure!(
                    "duplicate_id",
                    !world.entities.contains_key(&entity.id),
                    "duplicate entity ID: {}",
                    entity.id
                );
                world::record_id(std::str::from_utf8(path)?, paths::ENTITIES, &entity.id)?;
                world.entities.insert(entity.id.clone(), entity);
            } else if path.starts_with(b"realm/") {
                crate::bail!(
                    "invalid_layout",
                    "unsupported realm path: {}",
                    path.as_bstr()
                );
            }
        }
        world.validate()?;
        if check_lineage {
            self.validate_lineage(&world.realm)?;
        }
        self.cache.borrow_mut().flush()?;
        Ok(world)
    }
    /// Read per-model build records and validate freshness against `world`.
    /// With `reusable`, try conflict stages and omit stale/missing records for rebuilding.
    pub fn records(&self, world: &World, reusable: bool) -> Result<BTreeMap<String, Build>> {
        let mut records = BTreeMap::new();
        for id in world.models.keys() {
            let path = paths::build(id);
            let mut candidates = Vec::new();
            match self.read(path.as_bytes(), limits::JSON_BYTES) {
                Ok(data) => candidates.push(data),
                Err(error) if !reusable => return Err(error),
                Err(_) => {}
            }
            if reusable {
                if let Some(entries) = self.conflicts.get(path.as_bytes()) {
                    for entry in entries {
                        if let Ok(data) = self.read_entry(entry, limits::JSON_BYTES) {
                            candidates.push(data);
                        }
                    }
                }
            }
            for data in candidates {
                let result = json::parse::<Build>(&data).and_then(|r| {
                    r.validate(&world.realm.profile, &world.models[id], &world.inputs[id])
                        .map(|()| r)
                });
                match result {
                    Ok(record) => {
                        records.insert(id.clone(), record);
                        break;
                    }
                    Err(error) if !reusable => return Err(error.into()),
                    Err(_) => {}
                }
            }
        }
        Ok(records)
    }
    /// Require complete Git history and identical realm rules in every realm-bearing ancestor.
    /// Cache canonical hashes by immutable commit OID, never by mutable branch name.
    pub fn validate_lineage(&self, realm: &Realm) -> Result<()> {
        ensure!(
            "shallow_history",
            text(&self.repo.run(["rev-parse", "--is-shallow-repository"])?)? == "false",
            "realm lineage requires complete Git history; fetch --unshallow first"
        );
        let parents = self.repo.parents(self.mode.commit())?;
        if parents.is_empty() {
            return Ok(());
        }
        let mut args = vec!["rev-list".to_owned()];
        args.extend(parents);
        let commits = self.repo.run(args)?;
        let cache_path = io::local(&self.repo.root, "cache/lineage.json")?;
        let mut cache: BTreeMap<String, Option<String>> = io::read_cache(&cache_path);
        let expected = json::hash(realm)?;
        let mut changed = false;
        for oid in text(&commits)?.lines() {
            if !cache.contains_key(oid) {
                let bytes = self
                    .batch
                    .borrow_mut()
                    .read(&format!("{oid}:{}", paths::REALM), limits::JSON_BYTES)?;
                let hash = if let Some(bytes) = bytes {
                    let previous: Realm = json::parse(&bytes)?;
                    previous.validate()?;
                    Some(json::hash(&previous)?)
                } else {
                    None
                };
                cache.insert(oid.to_owned(), hash);
                changed = true;
            }
            ensure!(
                "realm_lineage",
                cache[oid].as_ref().is_none_or(|hash| hash == &expected),
                "immutable realm rules differ from ancestor {oid}"
            );
        }
        if changed {
            io::write_cache(&cache_path, &cache);
        }
        Ok(())
    }
    /// Stream or reuse a timestamp-validated descriptor within this snapshot only.
    /// Frozen build files and LFS object paths never enter the persistent source cache.
    pub fn hash_file(&self, path: &std::path::Path) -> Result<Descriptor> {
        self.cache.borrow_mut().transient_descriptor(path)
    }
    /// Hash actual worktree bytes using the persistent, stamp-validated source cache.
    /// Unlike `working_descriptor`, this never interprets pointer text as payload bytes.
    pub(crate) fn hash_working_file(&self, path: &std::path::Path) -> Result<Descriptor> {
        self.cache.borrow_mut().descriptor(path)
    }
    /// Best-effort persist this snapshot's source descriptor cache.
    pub fn flush(&self) -> Result<()> {
        self.cache.borrow_mut().flush()
    }
    /// Resolve a safe runtime/input path beneath the repository, rejecting symlinks.
    pub fn working_path(&self, path: &str) -> Result<PathBuf> {
        io::destination(&self.repo.root, path.as_bytes())
    }
}
