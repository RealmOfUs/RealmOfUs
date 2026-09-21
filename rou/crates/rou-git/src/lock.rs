//! PID-bearing writer locks with stale recovery and explicit signal cleanup.
use crate::{Context, Result, ensure, io, repository::Repository};
use fs2::FileExt;
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File},
    io::Write,
    path::PathBuf,
    sync::Arc,
};

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Owner {
    pid: u32,
    token: String,
}
struct State {
    path: PathBuf,
    token: String,
    _file: File,
}
impl State {
    fn cleanup(&self) {
        if let Ok(bytes) = io::read(&self.path, 4096) {
            if let Ok(owner) = rou_core::json::parse::<Owner>(&bytes) {
                if owner.token == self.token {
                    let _ = fs::remove_file(&self.path);
                }
            }
        }
    }
}

/// Owned author-operation lock; release only this instance's token on drop.
pub struct WriteLock {
    state: Arc<State>,
}
impl WriteLock {
    /// Acquire the repository writer lock with a PID/token and live-owner checks.
    /// Recover only provably stale owners; errors leave foreign locks untouched.
    pub fn acquire(repo: &Repository) -> Result<Self> {
        let path = io::local(&repo.root, "write.lock")?;
        let directory = path.parent().context("lock has no parent")?;
        fs::create_dir_all(directory)?;
        let owner = Owner {
            pid: std::process::id(),
            token: uuid::Uuid::new_v4().simple().to_string(),
        };
        let mut temporary = tempfile::NamedTempFile::new_in(directory)?;
        temporary.write_all(&rou_core::json::pretty(&owner)?)?;
        temporary.as_file().sync_all()?;
        temporary.as_file().try_lock_exclusive()?;
        for _ in 0..3 {
            match fs::hard_link(temporary.path(), &path) {
                Ok(()) => {
                    return Ok(Self {
                        state: Arc::new(State {
                            path,
                            token: owner.token,
                            _file: temporary.as_file().try_clone()?,
                        }),
                    });
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                    io::reject_symlinks(&path)?;
                    let existing = File::open(&path)?;
                    existing
                        .try_lock_exclusive()
                        .context(crate::error::HostError::new(
                            "lock_busy",
                            "another rou writer is active",
                        ))?;
                    let previous: Owner = rou_core::json::parse(&io::read(&path, 4096)?)?;
                    ensure!(
                        "lock_busy",
                        !alive(previous.pid),
                        "another rou writer is active (PID {})",
                        previous.pid
                    );
                    // Recheck the inode before unlinking a stale owner's directory entry.
                    if io::Stamp::of(&existing.metadata()?) == io::Stamp::of(&fs::metadata(&path)?)
                    {
                        fs::remove_file(&path)?;
                    }
                }
                Err(error) => return Err(error.into()),
            }
        }
        crate::bail!(
            "lock_busy",
            "writer lock changed repeatedly; retry after the active writer finishes"
        )
    }
    /// Install process-wide SIGINT/SIGTERM cleanup for owned writer/index locks and exit with 130.
    /// Call once per CLI process after acquiring its writer lock.
    pub fn install_signal_cleanup(&self) -> Result<()> {
        let state = self.state.clone();
        ctrlc::set_handler(move || {
            let locks = INDEX_LOCKS
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            for lock in locks.iter() {
                lock.cleanup();
            }
            state.cleanup();
            std::process::exit(130);
        })?;
        Ok(())
    }
}
impl Drop for WriteLock {
    fn drop(&mut self) {
        self.state.cleanup();
    }
}

#[cfg(unix)]
fn alive(pid: u32) -> bool {
    use nix::{errno::Errno, sys::signal::kill, unistd::Pid};
    if pid == 0 || pid > i32::MAX as u32 {
        return false;
    }
    !matches!(kill(Pid::from_raw(pid as i32), None), Err(Errno::ESRCH))
}
#[cfg(not(unix))]
fn alive(_pid: u32) -> bool {
    true
}

// Only host-owned index locks are registered. The signal thread and the rename
// step share this mutex, so cleanup cannot unlink a later writer's lock.
static INDEX_LOCKS: std::sync::Mutex<Vec<Arc<IndexState>>> = std::sync::Mutex::new(Vec::new());
struct IndexState {
    path: PathBuf,
    file: File,
    temporary: std::sync::Mutex<Option<tempfile::TempDir>>,
}
impl IndexState {
    fn cleanup(&self) {
        // The signal handler calls process::exit, so Drop alone is insufficient.
        if let Some(directory) = self
            .temporary
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .take()
        {
            let _ = directory.close();
        }
        if let (Ok(owned), Ok(current)) = (self.file.metadata(), fs::symlink_metadata(&self.path)) {
            if !current.file_type().is_symlink() && io::Stamp::of(&owned) == io::Stamp::of(&current)
            {
                let _ = fs::remove_file(&self.path);
            }
        }
    }
}

pub(crate) struct IndexLock {
    state: Arc<IndexState>,
    file: File,
}
impl IndexLock {
    pub(crate) fn acquire(path: PathBuf) -> Result<Self> {
        let mut locks = INDEX_LOCKS
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .context(crate::error::HostError::new(
                "lock_busy",
                "Git index is locked by another operation",
            ))?;
        let state = Arc::new(IndexState {
            path,
            file: file.try_clone()?,
            temporary: std::sync::Mutex::new(None),
        });
        locks.push(state.clone());
        Ok(Self { state, file })
    }
    pub(crate) fn file_mut(&mut self) -> &mut File {
        &mut self.file
    }
    pub(crate) fn temporary_index(&self) -> Result<PathBuf> {
        let mut temporary = self
            .state
            .temporary
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let directory = tempfile::Builder::new().prefix("rou-index-").tempdir_in(
            self.state
                .path
                .parent()
                .context("index lock has no parent")?,
        )?;
        let path = directory.path().join("index");
        *temporary = Some(directory);
        Ok(path)
    }
    pub(crate) fn commit(&self, destination: &std::path::Path) -> Result<()> {
        let mut locks = INDEX_LOCKS
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        fs::rename(&self.state.path, destination)?;
        self.state.cleanup();
        locks.retain(|state| !Arc::ptr_eq(state, &self.state));
        Ok(())
    }
}
impl Drop for IndexLock {
    fn drop(&mut self) {
        let mut locks = INDEX_LOCKS
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        self.state.cleanup();
        locks.retain(|state| !Arc::ptr_eq(state, &self.state));
    }
}
