//! Resolve and verify LFS payloads without changing the authoring worktree.
use crate::{
    Result, ensure, io, repository,
    snapshot::{Mode, Snapshot},
};
use rou_core::{documents::Descriptor, lfs, limits};
use std::{fs, io::Write, path::PathBuf, process::Stdio};

#[derive(Debug)]
/// A verified descriptor whose payload is not available locally. The field names its path.
pub struct MissingPayload(pub String);
impl std::fmt::Display for MissingPayload {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "missing LFS payload: {}", self.0)
    }
}
impl std::error::Error for MissingPayload {}

impl Snapshot {
    /// Resolve Git LFS storage once per snapshot. A new snapshot observes configuration changes.
    pub fn media_dir(&self) -> Result<PathBuf> {
        if let Some(path) = self.media.borrow().as_ref() {
            return Ok(path.clone());
        }
        let environment = self.repo.run(["lfs", "env"])?;
        for line in environment.split(|c| *c == b'\n') {
            if let Some(path) = line.strip_prefix(b"LocalMediaDir=") {
                let path = repository::os_path(path)?;
                let path = if path.is_absolute() {
                    path
                } else {
                    self.repo.root.join(path)
                };
                *self.media.borrow_mut() = Some(path.clone());
                return Ok(path);
            }
        }
        crate::bail!(
            "invalid_lfs_config",
            "Git LFS did not report its object directory"
        )
    }

    /// Find verified bytes in the worktree or LFS storage; optionally fetch a missing object.
    /// Return [`MissingPayload`] when absent and `fetch` is false; never materialize the worktree.
    pub fn payload_path(&self, path: &str, wanted: &Descriptor, fetch: bool) -> Result<PathBuf> {
        wanted.validate_shape()?;
        if self.mode == Mode::Worktree {
            let file = self.working_path(path)?;
            if file.is_file()
                && file.metadata()?.len() == wanted.bytes
                && self.hash_working_file(&file)? == *wanted
            {
                return Ok(file);
            }
        }
        let cache = self
            .media_dir()?
            .join(&wanted.sha256[..2])
            .join(&wanted.sha256[2..4])
            .join(&wanted.sha256);
        io::reject_symlinks(&cache)?;
        if cache.exists() {
            ensure!(
                "payload_mismatch",
                self.hash_file(&cache)? == *wanted,
                "LFS cache hash mismatch: {path}"
            );
            return Ok(cache);
        }
        if !fetch {
            return Err(MissingPayload(path.into()).into());
        }
        let mut cmd = repository::command(&self.repo.root);
        cmd.args([
            "-c",
            "lfs.fetchinclude=",
            "-c",
            "lfs.fetchexclude=",
            "lfs",
            "smudge",
            "--",
            path,
        ])
        .env("GIT_LFS_SKIP_SMUDGE", "0")
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped());
        let mut process = cmd.spawn()?;
        process
            .stdin
            .take()
            .expect("piped pointer input")
            .write_all(&lfs::encode(wanted)?)?;
        let result = process.wait_with_output()?;
        ensure!(
            "fetch_failed",
            result.status.success(),
            "LFS fetch failed for {path}: {}",
            String::from_utf8_lossy(&result.stderr)
        );
        if !cache.exists() {
            return Err(MissingPayload(path.into()).into());
        }
        ensure!(
            "payload_mismatch",
            self.hash_file(&cache)? == *wanted,
            "LFS cache hash mismatch after fetch: {path}"
        );
        Ok(cache)
    }

    /// Read and verify a runtime GLB-sized payload, optionally fetching it into LFS storage.
    pub fn payload(&self, path: &str, wanted: &Descriptor, fetch: bool) -> Result<Vec<u8>> {
        let file = self.payload_path(path, wanted, fetch)?;
        let bytes = io::read(&file, limits::GLB_BYTES)?;
        wanted.verify(&bytes)?;
        Ok(bytes)
    }

    /// Copy verified source bytes into an isolated build input and verify the copy.
    pub fn freeze_payload(
        &self,
        path: &str,
        wanted: &Descriptor,
        target: &std::path::Path,
    ) -> Result<()> {
        let source = self.payload_path(path, wanted, true)?;
        io::reject_symlinks(target)?;
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::copy(source, target)?;
        ensure!(
            "file_changed",
            self.hash_file(target)? == *wanted,
            "source changed during freeze: {path}"
        );
        Ok(())
    }
}
