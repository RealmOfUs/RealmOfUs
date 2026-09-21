//! Produce a self-contained browse bundle from a validated snapshot.
use crate::{
    Context, Result, ensure, io,
    repository::{Repository, text},
    snapshot::Mode,
};
use rou_core::{
    documents::{Manifest, Version},
    formats, glb, json, limits, paths, spatial,
};
use serde::Serialize;
use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};

#[derive(Debug, Serialize)]
/// Location and version binding of a generated browse bundle.
pub struct ViewReport {
    /// True after the operation has completed successfully.
    pub ok: bool,
    /// Absolute path to the generated bundle's realm.json.
    pub manifest: PathBuf,
    /// Number of entities in the complete catalog.
    pub entity_count: usize,
    /// Selected entity IDs whose shared payloads were provided.
    pub downloaded_entities: Vec<String>,
    /// Version sidecar binding the generated catalog bytes to this snapshot.
    pub version: Version,
}

impl Repository {
    /// Generate a complete catalog with selected GLBs and a byte-bound version sidecar.
    /// Use a sibling temporary directory and preserve an existing bundle on replacement failure.
    /// Does not change authoring files or the index; `chunks` restricts payloads, not metadata.
    pub fn view(&self, mode: Mode, output: Option<&Path>, chunks: &[String]) -> Result<ViewReport> {
        let validated = self.validate(mode, false)?;
        let commit = validated.snapshot.mode.commit().unwrap_or("");
        let output = match output {
            Some(path) => std::path::absolute(path)?,
            None => io::local(
                &self.root,
                &format!(
                    "view/{}",
                    if commit.is_empty() {
                        "worktree"
                    } else {
                        commit
                    }
                ),
            )?,
        };
        // Resolve only the parent: bundle-external aliases such as /tmp are
        // allowed, but following the bundle root would hide a forbidden link.
        let name = output.file_name().ok_or_else(|| {
            crate::error::HostError::new("unsafe_path", "view output must name a directory")
        })?;
        let parent = output.parent().context("view output has no parent")?;
        fs::create_dir_all(parent)?;
        let output = parent.canonicalize()?.join(name);
        ensure!(
            "unsafe_path",
            output != self.root,
            "view output cannot overwrite authoring root"
        );
        check_output(&output)?;
        let parent = output.parent().context("view output has no parent")?;
        let temporary = tempfile::Builder::new()
            .prefix(".rou-view-")
            .tempdir_in(parent)?;
        let manifest = validated.view.manifest()?;
        let mut selected: BTreeSet<String> = if chunks.is_empty() {
            validated.world.entities.keys().cloned().collect()
        } else {
            BTreeSet::new()
        };
        if !chunks.is_empty() {
            for chunk in validated.world.realm.profile.range(chunks)? {
                if let Some(ids) = validated.view.chunks.get(&spatial::chunk_key(chunk)) {
                    selected.extend(ids.iter().cloned());
                }
            }
        }
        fs::create_dir(temporary.path().join("objects"))?;
        let mut provided = BTreeSet::new();
        for entity in &validated.view.entities {
            if selected.contains(&entity.id) && provided.insert(&entity.asset.sha256) {
                let bytes = validated.snapshot.payload(
                    &entity.asset.path,
                    &entity.asset.descriptor(),
                    true,
                )?;
                glb::precheck(&bytes)?;
                io::atomic_write(
                    &temporary.path().join(paths::object(&entity.asset.sha256)),
                    &bytes,
                )?;
            }
        }
        let catalog = json::pretty(&manifest)?;
        let version = Version {
            format: formats::GIT_VIEW.into(),
            realm_id: validated.world.realm.id.clone(),
            git_object_format: text(&self.run(["rev-parse", "--show-object-format"])?)?.into(),
            commit_oid: commit.into(),
            view_root_sha256: validated.view.root_hash()?,
            catalog_sha256: json::digest(&catalog),
            validation: "local-trusted".into(),
        };
        version.validate(&catalog)?;
        io::atomic_write(&temporary.path().join(paths::REALM), &catalog)?;
        io::write_json(&temporary.path().join(paths::VERSION), &version)?;
        validated.snapshot.flush()?;
        // Finish preparation before touching an existing bundle. Keep it if replacement fails.
        check_output(&output)?;
        let backup = parent.join(format!(
            ".rou-view-backup-{}",
            uuid::Uuid::new_v4().simple()
        ));
        let existed = output.exists();
        if existed {
            fs::rename(&output, &backup)?;
        }
        if let Err(error) = fs::rename(temporary.path(), &output) {
            if existed {
                let _ = fs::rename(&backup, &output);
            }
            return Err(error.into());
        }
        if existed {
            fs::remove_dir_all(&backup)?;
        }
        Ok(ViewReport {
            ok: true,
            manifest: output.join(paths::REALM),
            entity_count: manifest.entities.len(),
            downloaded_entities: selected.into_iter().collect(),
            version,
        })
    }
}

fn check_output(output: &Path) -> Result<()> {
    io::reject_symlinks(output)?;
    if !output.exists() {
        return Ok(());
    }
    ensure!(
        "invalid_layout",
        output.is_dir(),
        "view output must be a directory"
    );
    for entry in fs::read_dir(output)? {
        let entry = entry?;
        let name = entry.file_name();
        let kind = entry.file_type()?;
        ensure!(
            "unsafe_path",
            !kind.is_symlink(),
            "view output contains symlinks"
        );
        if name == ".DS_Store" && kind.is_file() {
            continue;
        }
        if name == "objects" {
            ensure!(
                "invalid_layout",
                kind.is_dir(),
                "objects must be a directory"
            );
            for object in fs::read_dir(entry.path())? {
                let object = object?;
                ensure!(
                    "invalid_layout",
                    object.file_type()?.is_file(),
                    "objects contains an unrelated entry"
                );
                let name = object.file_name();
                if name == ".DS_Store" {
                    continue;
                }
                let name = name.to_str().ok_or_else(|| {
                    crate::error::HostError::new("invalid_layout", "invalid object filename")
                })?;
                let hash = name.strip_suffix(".glb").ok_or_else(|| {
                    crate::error::HostError::new(
                        "invalid_layout",
                        "unrelated file in objects directory",
                    )
                })?;
                rou_core::documents::hash_shape(hash)?;
            }
        } else {
            ensure!(
                "invalid_layout",
                (name == paths::REALM || name == paths::VERSION) && kind.is_file(),
                "view output contains unrelated files"
            );
        }
    }
    let catalog = output.join(paths::REALM);
    if catalog.exists() {
        json::parse::<Manifest>(&io::read(&catalog, limits::JSON_BYTES)?)?.validate()?;
    }
    Ok(())
}
