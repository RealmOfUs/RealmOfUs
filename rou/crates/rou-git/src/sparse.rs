//! Apply local selection cones, protect departing author files and hydrate exact paths.
use crate::{
    Context, Result, ensure, io,
    repository::Repository,
    snapshot::{Mode, Snapshot},
};
use bstr::{BString, ByteSlice};
use rou_core::{
    json, lfs, limits,
    selection::{Resolved, Selection},
};
use serde::Serialize;
use std::{
    collections::BTreeSet,
    fs,
    io::{Read, Write},
};

#[derive(Debug, Serialize)]
/// Resolved selection and ignored author files preserved during a cone shrink.
pub struct SparseReport {
    /// Resolved cone, selected entities and exact LFS payload paths.
    pub resolution: Resolved,
    /// Absolute paths of ignored files moved to `.rou/backups/` before cone removal.
    pub backups: Vec<String>,
}

impl Repository {
    /// Read and validate saved selectors, defaulting to an empty metadata-only selection.
    pub fn selection(&self) -> Result<Selection> {
        let file = io::local(&self.root, "selection.json")?;
        let selection = if file.exists() {
            json::parse(&io::read(&file, limits::JSON_BYTES)?)?
        } else {
            Selection::default()
        };
        selection.validate()?;
        Ok(selection)
    }
    /// Validate a fixed commit and derive its selected cone and exact payload paths.
    pub fn resolve_selection(&self, revision: &str, selection: &Selection) -> Result<Resolved> {
        let validated = self.validate(Mode::Commit(revision.into()), false)?;
        Ok(selection.resolve(&validated.world, &validated.view)?)
    }
    /// Replace or extend saved selectors, protect shrinking directories, then hydrate selected files.
    /// Requires a writer lock and a conflict-free index; preserve unrelated author changes.
    pub fn sparse(&self, mode: &str, mut selection: Selection) -> Result<SparseReport> {
        selection.validate()?;
        ensure!(
            "invalid_selection",
            !selection.is_empty(),
            "sparse {mode} requires at least one selector"
        );
        match mode {
            "set" => {}
            "add" => {
                let old = self.selection()?;
                selection.all |= old.all;
                selection.chunks.extend(old.chunks);
                selection.entities.extend(old.entities);
            }
            _ => crate::bail!("invalid_selection", "sparse requires set, add, or list"),
        }
        if selection.all {
            selection.chunks.clear();
            selection.entities.clear();
        } else {
            selection.chunks.sort();
            selection.chunks.dedup();
            selection.entities.sort();
            selection.entities.dedup();
        }
        let head = self.resolve("HEAD")?;
        ensure!(
            "unresolved_conflict",
            self.run(["ls-files", "-u", "-z"])?.is_empty(),
            "resolve merge conflicts before changing sparse selection"
        );
        let resolution = self.resolve_selection(&head, &selection)?;
        let backups = self.prepare_selection(&head, &selection, &resolution)?;
        ensure!(
            "snapshot_changed",
            self.head()?.as_deref() == Some(&head),
            "HEAD changed during sparse checkout; inspect worktree and retry"
        );
        io::write_json(&io::local(&self.root, "selection.json")?, &selection)?;
        io::write_json(
            &io::local(&self.root, "readiness.json")?,
            &serde_json::json!({"commit":head,"resolution":resolution}),
        )?;
        Ok(SparseReport {
            resolution,
            backups,
        })
    }
    /// Apply a resolved selection to the worktree and hydrate its exact payload set.
    /// Return paths of ignored files backed up before a cone shrink.
    pub fn prepare_selection(
        &self,
        revision: &str,
        selection: &Selection,
        resolution: &Resolved,
    ) -> Result<Vec<String>> {
        self.apply_selection(revision, selection, resolution, false)
    }
    pub(crate) fn apply_selection(
        &self,
        revision: &str,
        selection: &Selection,
        resolution: &Resolved,
        protected: bool,
    ) -> Result<Vec<String>> {
        let backups = if selection.all {
            self.run(["sparse-checkout", "disable"])?;
            Vec::new()
        } else {
            let backups = if protected {
                Vec::new()
            } else {
                self.protect_sparse_shrink(&resolution.directories)?
            };
            let input = resolution.directories.join("\n") + "\n";
            self.input(
                ["sparse-checkout", "set", "--cone", "--stdin"],
                input.as_bytes(),
            )?;
            backups
        };
        self.hydrate(
            revision,
            &resolution
                .paths
                .iter()
                .chain(&resolution.assets)
                .cloned()
                .collect::<Vec<_>>(),
        )?;
        Ok(backups)
    }
    /// Re-resolve saved selectors at current HEAD and persist readiness; return any backup paths.
    pub fn refresh_selection(&self) -> Result<Vec<String>> {
        let Some(head) = self.head()? else {
            return Ok(Vec::new());
        };
        let selection = self.selection()?;
        let resolution = self.resolve_selection(&head, &selection)?;
        let backups = self.prepare_selection(&head, &selection, &resolution)?;
        io::write_json(
            &io::local(&self.root, "readiness.json")?,
            &serde_json::json!({"commit":head,"resolution":resolution}),
        )?;
        Ok(backups)
    }
    /// Reject dirty files and back up ignored files only in tracked directories leaving the cone.
    /// Unrelated out-of-cone directories are not moved or rejected.
    pub fn protect_sparse_shrink(&self, directories: &[String]) -> Result<Vec<String>> {
        // Git only removes directories containing tracked, currently included
        // files. Unrelated ignored directories (for example .godot) are safe.
        let mut removed = BTreeSet::<BString>::new();
        for item in self.run(["ls-files", "-t", "-z"])?.split(|c| *c == 0) {
            if item.len() < 3 || item[0] == b'S' {
                continue;
            }
            let path = &item[2..];
            if cone_contains(path, directories) {
                continue;
            }
            for (index, byte) in path.iter().enumerate() {
                if *byte == b'/'
                    && !cone_contains(
                        &path[..index + 1]
                            .iter()
                            .copied()
                            .chain(b"file".iter().copied())
                            .collect::<Vec<_>>(),
                        directories,
                    )
                {
                    removed.insert(path[..index + 1].into());
                    break;
                }
            }
        }
        let at_risk = |path: &[u8]| removed.iter().any(|prefix| path.starts_with(prefix));
        let mut dirty = BTreeSet::<BString>::new();
        for args in [
            vec!["diff", "--name-only", "-z"],
            vec!["diff", "--cached", "--name-only", "-z"],
            vec!["ls-files", "--others", "--exclude-standard", "-z"],
        ] {
            dirty.extend(
                self.run(args)?
                    .split(|c| *c == 0)
                    .filter(|p| !p.is_empty())
                    .map(BString::from),
            );
        }
        for path in dirty {
            ensure!(
                "local_changes",
                !at_risk(&path),
                "sparse shrink would remove modified author file: {}",
                path.as_bstr()
            );
        }
        let ignored = self.run([
            "ls-files",
            "--others",
            "--ignored",
            "--exclude-standard",
            "-z",
        ])?;
        let mut backups = Vec::new();
        let run = uuid::Uuid::new_v4().simple().to_string();
        for path in ignored
            .split(|c| *c == 0)
            .filter(|p| !p.is_empty() && !p.starts_with(b".rou/"))
        {
            if !at_risk(path) {
                continue;
            }
            let source = io::destination(&self.root, path)?;
            // An ignored author file is backed up exactly, never discarded by cone checkout.
            ensure!(
                "unsafe_path",
                source.is_file(),
                "ignored path is not a regular file: {}",
                source.display()
            );
            let backup_root = io::local(&self.root, &format!("backups/{run}"))?;
            let destination = io::destination(&backup_root, path)?;
            fs::create_dir_all(destination.parent().context("backup has no parent")?)?;
            fs::rename(&source, &destination)?;
            backups.push(destination.to_string_lossy().into_owned());
        }
        Ok(backups)
    }
    /// Materialize selected LFS pointers after checking target identity and local changes.
    /// Refresh only matching index stat entries under Git's native index lock.
    pub fn hydrate(&self, revision: &str, paths: &[String]) -> Result<()> {
        let snapshot = Snapshot::new(self, Mode::Commit(revision.into()))?;
        let mut hydrated = Vec::new();
        let modified: BTreeSet<BString> = self
            .run(["diff", "--name-only", "-z", revision, "--"])?
            .split(|c| *c == 0)
            .filter(|path| !path.is_empty())
            .map(BString::from)
            .collect();
        for path in paths {
            let pointer = lfs::pointer(&snapshot.read(path.as_bytes(), limits::POINTER_BYTES)?)?
                .context("selected payload must be tracked by LFS")?;
            let destination = io::destination(&self.root, path.as_bytes())?;
            if destination.is_file() {
                let mut prefix = Vec::new();
                fs::File::open(&destination)?
                    .take(limits::POINTER_BYTES as u64 + 1)
                    .read_to_end(&mut prefix)?;
                if let Some(actual) = lfs::pointer(&prefix)? {
                    ensure!(
                        "local_changes",
                        actual == pointer,
                        "selected pointer differs from target; preserve local changes: {path}"
                    );
                } else {
                    ensure!(
                        "local_changes",
                        modified.contains(path.as_bytes())
                            || snapshot.hash_file(&destination)? == pointer,
                        "selected payload does not match target: {path}"
                    );
                    continue;
                }
            }
            ensure!(
                "local_changes",
                !modified.contains(path.as_bytes()),
                "selected path has local changes; preserve or restore it before materialization: {path}"
            );
            let payload = snapshot.payload_path(path, &pointer, true)?;
            let parent = destination.parent().context("payload has no parent")?;
            fs::create_dir_all(parent)?;
            let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
            std::io::copy(&mut fs::File::open(payload)?, temporary.as_file_mut())?;
            temporary.flush()?;
            ensure!(
                "payload_mismatch",
                snapshot.hash_file(temporary.path())? == pointer,
                "materialized payload hash mismatch: {path}"
            );
            io::reject_symlinks(&destination)?;
            temporary.persist(&destination).map_err(|e| e.error)?;
            hydrated.push(path.clone());
        }
        self.refresh_hydrated(&snapshot, &hydrated)?;
        snapshot.flush()
    }

    fn refresh_hydrated(&self, target: &Snapshot, paths: &[String]) -> Result<()> {
        if paths.is_empty() {
            return Ok(());
        }
        // Hold Git's native index lock and refresh only matching payload stats.
        // `add --refresh` never stages content, including concurrent author edits.
        let index = self.git_dir.join("index");
        let mut lock = crate::lock::IndexLock::acquire(self.git_dir.join("index.lock"))?;
        // Let Git replace a separate temporary index, keeping the owned lock's
        // inode stable until the final atomic replacement of the real index.
        let temporary_index = lock.temporary_index()?;
        fs::copy(&index, &temporary_index)?;
        let current = Snapshot::new(self, Mode::Index)?;
        let mut input = Vec::new();
        let mut names = Vec::new();
        for path in paths {
            let actual = current
                .entries
                .get(path.as_bytes())
                .context("materialized index path disappeared")?;
            let expected = &target.entries[path.as_bytes()];
            ensure!(
                "snapshot_changed",
                actual.oid == expected.oid && actual.mode == expected.mode,
                "index changed during materialization: {path}"
            );
            input.extend_from_slice(
                format!("{} {}\t{}\0", actual.mode, actual.oid, path).as_bytes(),
            );
            names.extend_from_slice(path.as_bytes());
            names.push(0);
        }
        // Pointer and payload lengths differ. Clear the old pointer stat first
        // so Git checks the clean-filter result before storing the payload stat.
        let mut reset = crate::repository::command(&self.root);
        reset
            .env("GIT_INDEX_FILE", &temporary_index)
            .args(["update-index", "-z", "--index-info"]);
        crate::repository::checked(crate::repository::execute_command(reset, Some(&input))?)?;
        let mut cmd = crate::repository::command(&self.root);
        cmd.env("GIT_INDEX_FILE", &temporary_index).args([
            "--literal-pathspecs",
            "add",
            "--refresh",
            "--pathspec-from-file=-",
            "--pathspec-file-nul",
        ]);
        crate::repository::checked(crate::repository::execute_command(cmd, Some(&names))?)?;
        std::io::copy(&mut fs::File::open(temporary_index)?, lock.file_mut())?;
        lock.file_mut().sync_all()?;
        lock.commit(&index)?;
        Ok(())
    }
}

/// Cone mode keeps root files and the immediate files of each selected ancestor.
pub fn cone_contains(path: &[u8], directories: &[String]) -> bool {
    let Some((parent, _)) = path.rsplit_once_str(b"/") else {
        return true;
    };
    directories.iter().any(|directory| {
        let directory = directory.as_bytes();
        path == directory
            || path.starts_with(&[directory, b"/"].concat())
            || parent == directory
            || directory.starts_with(&[parent, b"/"].concat())
    })
}
