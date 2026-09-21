//! Git-oriented status, diff, commit, branch, merge and publication workflows.
use crate::{
    Context, Result, ensure, io,
    repository::{Repository, text},
    snapshot::{Mode, Snapshot},
};
use rou_core::{selection::Selection, world};
use serde_json::{Value, json};
use std::ffi::OsString;

impl Repository {
    /// Return Git status, selectors and LFS presence without domain validation.
    pub fn status_json(&self) -> Result<Value> {
        let status = self.run([
            "status",
            "--porcelain=v2",
            "--branch",
            "--untracked-files=all",
        ])?;
        let lfs = self.run(["lfs", "ls-files", "--long"])?;
        let legend = "* means materialized; - means pointer or absent; \
            presence is not integrity validation";
        Ok(json!({
            "root": self.root,
            "head": self.head()?,
            "git_status": String::from_utf8_lossy(&status),
            "selection": self.selection()?,
            "domain_validation": "not_run",
            "lfs_worktree": String::from_utf8_lossy(&lfs),
            "lfs_legend": legend,
        }))
    }
    /// Return a concise Git/selection summary without domain validation.
    pub fn status_text(&self) -> Result<String> {
        let output = self.run(["status", "--short", "--branch"])?;
        let selection = self.selection()?;
        Ok(format!(
            "{}Selection: {}\nDomain validation: not run\n",
            String::from_utf8_lossy(&output),
            if selection.all {
                "all current entities".into()
            } else {
                format!(
                    "{} chunk selectors, {} explicit entities",
                    selection.chunks.len(),
                    selection.entities.len()
                )
            }
        ))
    }
    /// Validate the actual staged tree, commit it with Git, then validate the created commit.
    /// Detect concurrent HEAD/index changes; a post-commit failure retains the commit for inspection.
    pub fn commit(&self, message: &str, author: Option<&str>) -> Result<Value> {
        ensure!(
            "invalid_argument",
            !message.is_empty(),
            "commit requires -m MESSAGE"
        );
        let before = self.head()?;
        let tree = self.run(["write-tree"])?;
        self.validate(Mode::Index, true)?;
        ensure!(
            "snapshot_changed",
            self.head()? == before && self.run(["write-tree"])? == tree,
            "Git index or HEAD changed during validation"
        );
        let mut args = vec!["commit", "-m", message];
        if let Some(author) = author {
            args.extend(["--author", author]);
        }
        let output = self.run(args)?;
        let after = self.head()?.context("Git commit returned without HEAD")?;
        let validated = self.validate(Mode::Commit(after.clone()), true).with_context(|| {
            format!("Git created commit {after}, but its Realm validation failed; the commit exists and was not rolled back")
        })?;
        ensure!(
            "snapshot_changed",
            self.run(["rev-parse", &format!("{after}^{{tree}}")])? == tree,
            "Git created commit {after} with a different tree; inspect the existing commit"
        );
        Ok(json!({
            "commit": after,
            "git_output": String::from_utf8_lossy(&output),
            "validation": validated.report,
        }))
    }
    /// Switch/create a branch or detached version after validating its fixed target OID.
    /// Protect a shrinking cone once and reapply selectors after checking the resulting HEAD.
    pub fn switch(
        &self,
        reference: &str,
        new_branch: Option<&str>,
        checkout: bool,
        selection: Option<&Selection>,
    ) -> Result<Value> {
        if let Some(branch) = new_branch {
            self.run(["check-ref-format", "--branch", branch])?;
        }
        let target = match self.resolve(reference) {
            Ok(target) => target,
            Err(error) => {
                if new_branch.is_none() && !checkout && !reference.starts_with('-') {
                    self.resolve(&format!(
                        "refs/remotes/{}/{}",
                        self.default_remote()?,
                        reference
                    ))?
                } else {
                    return Err(error);
                }
            }
        };
        let before = self.head()?;
        if before.as_deref() != Some(&target) {
            self.validate(Mode::Commit(target.clone()), true)?;
        }
        let requested = selection.cloned().unwrap_or(self.selection()?);
        let mut backups = Vec::new();
        let mut prepared = None;
        if before.as_deref() != Some(&target) || selection.is_some() {
            let resolution = self.resolve_selection(&target, &requested)?;
            if !requested.all {
                backups = self.protect_sparse_shrink(&resolution.directories)?;
            }
            ensure!(
                "snapshot_changed",
                self.head()? == before,
                "HEAD changed during switch preparation"
            );
            prepared = Some(resolution);
        }
        let args = if let Some(branch) = new_branch {
            vec!["switch", "-c", branch, &target]
        } else if checkout {
            if self
                .status([
                    "show-ref",
                    "--verify",
                    "--quiet",
                    &format!("refs/heads/{reference}"),
                ])?
                .status
                .success()
            {
                vec!["switch", "--", reference]
            } else if reference == "HEAD" {
                vec!["checkout", "HEAD"]
            } else {
                vec!["checkout", "--detach", &target]
            }
        } else {
            ensure!(
                "invalid_argument",
                reference != "HEAD" && !reference.starts_with('-'),
                "switch requires a branch name; use checkout COMMIT for detached inspection"
            );
            vec!["switch", "--", reference]
        };
        let output = self.run(args)?;
        ensure!(
            "snapshot_changed",
            self.head()?.as_deref() == Some(&target),
            "branch changed during switch; inspect HEAD"
        );
        if selection.is_some() {
            io::write_json(&io::local(&self.root, "selection.json")?, &requested)?;
        }
        if let Some(resolution) = prepared {
            self.apply_selection(&target, &requested, &resolution, true)?;
            io::write_json(
                &io::local(&self.root, "readiness.json")?,
                &json!({"commit": target, "resolution": resolution}),
            )?;
        }
        Ok(json!({"head":target,"git_output":String::from_utf8_lossy(&output),"backups":backups}))
    }
    /// Validate the target, fast-forward where possible, or leave a divergent merge uncommitted.
    /// Support native merge abort and refresh selection after successful fast-forward/abort.
    pub fn merge(&self, reference: Option<&str>, ff_only: bool, abort: bool) -> Result<Value> {
        if abort {
            let output = self.run(["merge", "--abort"])?;
            let backups = self.refresh_selection()?;
            return Ok(json!({"git_output":String::from_utf8_lossy(&output),"backups":backups}));
        }
        let target = self.resolve(reference.context("merge requires a revision")?)?;
        let head = self.resolve("HEAD")?;
        self.validate(Mode::Commit(target.clone()), true)?;
        ensure!(
            "snapshot_changed",
            self.head()?.as_deref() == Some(&head),
            "HEAD changed while validating merge target"
        );
        let forward = self
            .status(["merge-base", "--is-ancestor", &head, &target])?
            .status
            .success();
        let merged = self
            .status(["merge-base", "--is-ancestor", &target, &head])?
            .status
            .success();
        let output = if forward || merged || ff_only {
            let output = self.run(["merge", "--ff-only", &target])?;
            self.refresh_selection()?;
            output
        } else {
            self.run(["merge", "--no-commit", "--no-ff", &target])?
        };
        Ok(json!({"git_output":String::from_utf8_lossy(&output),"head":self.head()?}))
    }
    /// Fetch an explicit/current branch and merge only by fast-forward.
    pub fn pull(&self, remote: Option<&str>, branch: Option<&str>) -> Result<Value> {
        let remote = remote.map(str::to_owned).unwrap_or(self.default_remote()?);
        ensure!(
            "invalid_argument",
            !remote.starts_with('-'),
            "invalid remote"
        );
        let branch = if let Some(branch) = branch {
            branch.to_owned()
        } else {
            let current = self.run(["symbolic-ref", "--short", "-q", "HEAD"])?;
            let current = text(&current)?;
            let configured =
                self.status(["config", "--get", &format!("branch.{current}.merge")])?;
            if configured.status.success() {
                text(&configured.stdout)?
                    .trim_start_matches("refs/heads/")
                    .to_owned()
            } else {
                current.into()
            }
        };
        self.run(["check-ref-format", &format!("refs/heads/{branch}")])?;
        self.run(["fetch", &remote, &branch])?;
        let target = self.resolve("FETCH_HEAD")?;
        self.merge(Some(&target), true, false)
    }
    /// Validate and push a fixed commit OID with its complete reachable LFS history.
    /// Restore missing historical payloads if needed; setting upstream does not change the fixed refspec.
    pub fn push(
        &self,
        remote: Option<&str>,
        branch: Option<&str>,
        upstream: bool,
    ) -> Result<Value> {
        let remote = remote.map(str::to_owned).unwrap_or(self.default_remote()?);
        ensure!(
            "invalid_argument",
            !remote.starts_with('-'),
            "invalid remote"
        );
        let branch = if let Some(branch) = branch {
            branch.to_owned()
        } else {
            text(&self.run(["symbolic-ref", "--short", "-q", "HEAD"])?)?.to_owned()
        };
        self.run(["check-ref-format", &format!("refs/heads/{branch}")])?;
        let target = self.resolve(&format!("refs/heads/{branch}"))?;
        self.validate(Mode::Commit(target.clone()), true)?;
        let upload = [
            "-c",
            "lfs.allowincompletepush=false",
            "lfs",
            "push",
            "--all",
            &remote,
            &target,
        ];
        if let Err(error) = self.run(upload) {
            let message = error.to_string().to_lowercase();
            ensure!(
                "missing_payload",
                message.contains("missing")
                    || message.contains("unable to find source")
                    || message.contains("no such file"),
                "cannot publish required LFS history: {error}"
            );
            self.run(["lfs", "fetch", "--all", &self.default_remote()?, &target])?;
            self.run(upload)?;
        }
        let output = self.run([
            "-c",
            "lfs.allowincompletepush=false",
            "push",
            &remote,
            &format!("{target}:refs/heads/{branch}"),
        ])?;
        if upstream {
            self.run(["config", &format!("branch.{branch}.remote"), &remote])?;
            self.run([
                "config",
                &format!("branch.{branch}.merge"),
                &format!("refs/heads/{branch}"),
            ])?;
        }
        Ok(json!({"commit":target,"git_output":String::from_utf8_lossy(&output)}))
    }
    /// Compute a semantic world diff for optional staged/revision selectors without changing files.
    pub fn domain_diff(&self, args: &[OsString]) -> Result<Value> {
        let mut staged = false;
        let mut refs = Vec::new();
        for argument in args {
            let argument = argument.to_str().context("revision must be UTF-8")?;
            if matches!(argument, "--staged" | "--cached") {
                staged = true;
            } else {
                ensure!(
                    "invalid_argument",
                    !argument.starts_with('-'),
                    "diff --realm supports [--staged] [A [B]]"
                );
                refs.push(argument);
            }
        }
        ensure!(
            "invalid_argument",
            refs.len() <= 2 && (!staged || refs.len() <= 1),
            "diff --realm supports [A [B]] or --staged [A]"
        );
        let mut before = if staged {
            self.head()?.map(Mode::Commit)
        } else {
            Some(Mode::Index)
        };
        let mut after = if staged { Mode::Index } else { Mode::Worktree };
        if let Some(reference) = refs.first() {
            before = Some(Mode::Commit(self.resolve(reference)?));
        }
        if let Some(reference) = refs.get(1) {
            after = Mode::Commit(self.resolve(reference)?);
        }
        let old = before
            .as_ref()
            .map(|mode| Snapshot::new(self, mode.clone()).and_then(|s| s.world(false)))
            .transpose()?;
        let new = Snapshot::new(self, after.clone())?.world(false)?;
        let mut result = world::diff(old.as_ref(), Some(&new))?;
        result["from"] = json!(before.as_ref().map_or("EMPTY", Mode::label));
        result["to"] = json!(after.label());
        Ok(result)
    }
}
