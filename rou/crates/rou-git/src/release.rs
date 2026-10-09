//! Create and inspect browse releases: annotated Git tags that bind a view root.
//!
//! Publishing is explicit and uses Git itself: `rou push` for the branch and its LFS
//! objects, then `git push <remote> <tag>`. Viewers derive the browse catalog from
//! the tagged tree, so nothing is uploaded besides ordinary Git refs and objects.
use crate::{Repository, Result, ensure, repository::text, snapshot::Mode};
use rou_core::online;
use serde::Serialize;

/// A local annotated tag carrying a valid browse view-root binding.
#[derive(Debug, Clone, Serialize)]
pub struct BrowseTag {
    /// Git tag name.
    pub tag: String,
    /// Commit selected by the annotated tag.
    pub commit_oid: String,
    /// View root SHA-256 from the tag annotation.
    pub view_root_sha256: String,
}

/// Result of creating or verifying a local browse tag.
#[derive(Debug, Serialize)]
pub struct ReleaseReport {
    /// True once the annotated tag exists and binds the validated view root.
    pub ok: bool,
    /// Git tag name.
    pub tag: String,
    /// Commit selected by the tag.
    pub commit_oid: String,
    /// View root bound by the annotated tag.
    pub view_root_sha256: String,
    /// Viewer address derived from the default remote when it is an HTTP(S) forge URL.
    pub address: Option<String>,
    /// Git remote the address was derived from, when any.
    pub remote: Option<String>,
}

impl Repository {
    /// List local annotated tags that contain exactly one valid `view-root:` line.
    pub fn release_list(&self) -> Result<Vec<BrowseTag>> {
        let names = self.run(["tag", "--list"])?;
        let mut result = Vec::new();
        for tag in text(&names)?.lines().filter(|tag| !tag.is_empty()) {
            let reference = format!("refs/tags/{tag}");
            let kind = self.run(["cat-file", "-t", &reference])?;
            if text(&kind)? != "tag" {
                continue;
            }
            let contents = self.run(["for-each-ref", "--format=%(contents)", &reference])?;
            let Some(view_root_sha256) =
                online::parse_release_annotation(text(&contents)?.trim_end())?
            else {
                continue;
            };
            result.push(BrowseTag {
                tag: tag.to_owned(),
                commit_oid: self.resolve(tag)?,
                view_root_sha256,
            });
        }
        result.sort_by(|a, b| a.tag.cmp(&b.tag));
        Ok(result)
    }

    /// Validate `HEAD` and create (or verify) a local annotated tag binding its view root.
    ///
    /// Nothing is pushed or uploaded: publish with `rou push` and `git push <remote> <tag>`.
    /// Repeating the command verifies that an existing tag selects the same commit and view root.
    pub fn release_create(&self, tag: &str) -> Result<ReleaseReport> {
        online::validate_release_tag(tag)?;
        let commit = self.resolve("HEAD")?;
        let validated = self.validate(Mode::Commit(commit.clone()), true)?;
        let view_root = validated.view.root_hash()?;
        let annotation = online::release_annotation(&view_root)?;
        self.ensure_release_tag(tag, &commit, &annotation, &view_root)?;
        let (remote, address) = match self.default_remote() {
            Ok(remote) => {
                let address = self
                    .run(["remote", "get-url", &remote])
                    .ok()
                    .and_then(|raw| text(&raw).ok().map(str::to_owned))
                    .and_then(|url| viewer_address(url.trim(), tag));
                (Some(remote), address)
            }
            Err(_) => (None, None),
        };
        Ok(ReleaseReport {
            ok: true,
            tag: tag.to_owned(),
            commit_oid: commit,
            view_root_sha256: view_root,
            address,
            remote,
        })
    }

    fn ensure_release_tag(
        &self,
        tag: &str,
        commit: &str,
        annotation: &str,
        view_root: &str,
    ) -> Result<()> {
        let reference = format!("refs/tags/{tag}");
        let exists = self
            .status(["show-ref", "--verify", "--quiet", &reference])?
            .status
            .success();
        if !exists {
            let message = format!("Realm of Us browse release {tag}\n\n{annotation}");
            self.run_visible(["tag", "-a", tag, "-m", &message, commit])?;
            return Ok(());
        }
        let kind = self.run(["cat-file", "-t", &reference])?;
        ensure!(
            "release_conflict",
            text(&kind)? == "tag",
            "release tag exists but is not annotated: {tag}"
        );
        ensure!(
            "release_conflict",
            self.resolve(tag)? == commit,
            "release tag {tag} selects a different commit"
        );
        let contents = self.run(["for-each-ref", "--format=%(contents)", &reference])?;
        let bound = online::parse_release_annotation(text(&contents)?.trim_end())?;
        ensure!(
            "release_conflict",
            bound.as_deref() == Some(view_root),
            "release tag {tag} binds a different view root"
        );
        Ok(())
    }
}

/// Derive `rou://host/owner/repository@tag` from an HTTP(S) forge remote URL.
///
/// Credentials, a `.git` suffix and a trailing slash are dropped; other remote
/// shapes (SSH, local paths) yield `None` because viewers cannot reach them.
fn viewer_address(url: &str, tag: &str) -> Option<String> {
    let rest = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))?;
    let (authority, path) = rest.split_once('/')?;
    let host = authority.rsplit('@').next()?;
    let path = path.trim_end_matches('/');
    let path = path.strip_suffix(".git").unwrap_or(path);
    let (owner, repository) = path.split_once('/')?;
    if host.is_empty() || owner.is_empty() || repository.is_empty() || repository.contains('/') {
        return None;
    }
    let address = format!("rou://{host}/{owner}/{repository}@{tag}");
    online::parse_source(&address).ok().map(|_| address)
}

#[cfg(test)]
mod tests {
    use super::viewer_address;

    #[test]
    fn viewer_addresses_come_only_from_http_forge_remotes() {
        assert_eq!(
            viewer_address("https://user:secret@forge.example/realm/garden.git/", "v1").as_deref(),
            Some("rou://forge.example/realm/garden@v1")
        );
        assert_eq!(
            viewer_address("git@forge.example:realm/garden.git", "v1"),
            None
        );
        assert_eq!(viewer_address("/tmp/garden.git", "v1"), None);
        assert_eq!(viewer_address("https://forge.example/garden", "v1"), None);
    }
}
