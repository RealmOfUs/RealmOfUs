//! Produce a self-contained browse bundle from a validated snapshot.
use crate::{
    Context, Result, ensure, io,
    payload::MissingPayload,
    repository::{Repository, text},
    snapshot::Mode,
};
use rou_core::{
    documents::{Manifest, Version},
    formats, glb, image, json, limits, paths,
    selection::Selection,
};
use serde::Serialize;
use std::{
    collections::{BTreeMap, BTreeSet},
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
    /// Number of distinct models referenced by the catalog.
    #[serde(skip)]
    pub model_count: usize,
    /// Number of distinct models whose payloads are present in the bundle.
    #[serde(skip)]
    pub included_models: usize,
}

impl Repository {
    /// Generate a complete catalog with selected GLBs and a byte-bound version sidecar.
    /// Use a sibling temporary directory and preserve an existing bundle on replacement failure.
    /// Does not change authoring files or the index. Explicit selectors may fetch runtime payloads;
    /// absent selectors use the saved selection and only locally available bytes, never the network.
    /// Verified worktree GLBs are hard-linked when possible; other payloads are copied.
    pub fn view(
        &self,
        mode: Mode,
        output: Option<&Path>,
        selection: Option<&Selection>,
    ) -> Result<ViewReport> {
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
        let preview = validated.view.manifest()?;
        let bundle = preview.bundle()?;
        let manifest = &bundle.root;
        let fetch = selection.is_some();
        let selection = match selection {
            Some(selection) => selection.clone(),
            None => self.selection()?,
        };
        let selected: BTreeSet<_> = selection
            .resolve(&validated.world, &validated.view)?
            .entity_ids
            .into_iter()
            .collect();
        fs::create_dir(temporary.path().join("objects"))?;
        let mut provided = BTreeSet::new();
        let mut glbs = BTreeMap::new();
        let mut images = BTreeMap::new();
        for entity in &validated.view.entities {
            if !selected.contains(&entity.id) {
                continue;
            }
            for (path, descriptor) in entity.payloads() {
                let object = bundle_object(path, &descriptor.sha256);
                if !provided.contains(&object) {
                    let working = validated.snapshot.working_path(path)?;
                    let local = working.is_file()
                        && working.metadata()?.len() == descriptor.bytes
                        && validated.snapshot.hash_working_file(&working)? == descriptor;
                    let source = if local {
                        working
                    } else {
                        match validated.snapshot.payload_path(path, &descriptor, fetch) {
                            Ok(path) => path,
                            Err(error)
                                if !fetch && error.downcast_ref::<MissingPayload>().is_some() =>
                            {
                                continue;
                            }
                            Err(error) => return Err(error),
                        }
                    };
                    let destination = temporary.path().join(&object);
                    if !local || fs::hard_link(&source, &destination).is_err() {
                        fs::copy(&source, &destination)?;
                    }
                    // Verify the published inode or copy, not only the earlier source snapshot.
                    let bytes = io::read(&destination, limits::GLB_BYTES)?;
                    descriptor.verify(&bytes)?;
                    if path.ends_with(".glb") {
                        glbs.insert(object.clone(), glb::precheck(&bytes)?);
                    } else {
                        let report = image::precheck(&bytes)?;
                        ensure!(
                            "invalid_image",
                            path.ends_with(&format!(".{}", report.extension)),
                            "image signature and extension differ"
                        );
                        images.insert(object.clone(), report);
                    }
                    provided.insert(object);
                }
            }
        }

        for entity in &preview.entities {
            for (asset, textures) in [
                (&entity.asset, &entity.textures.asset),
                (&entity.lod1, &entity.textures.lod1),
            ] {
                if let Some(report) = glbs.get(&asset.path) {
                    report.match_textures(textures)?;
                    if textures
                        .iter()
                        .all(|texture| images.contains_key(&texture.path))
                    {
                        report.clone().with_images(
                            &textures
                                .iter()
                                .map(|texture| {
                                    (texture.filename().to_owned(), images[&texture.path].clone())
                                })
                                .collect(),
                        )?;
                    }
                }
            }
        }
        let included: BTreeSet<_> = validated
            .view
            .entities
            .iter()
            .filter(|entity| {
                selected.contains(&entity.id)
                    && entity.payloads().all(|(path, descriptor)| {
                        provided.contains(&bundle_object(path, &descriptor.sha256))
                    })
            })
            .map(|entity| entity.id.clone())
            .collect();
        let model_count = validated
            .world
            .entities
            .values()
            .map(|entity| &entity.model)
            .collect::<BTreeSet<_>>()
            .len();
        let included_models = validated
            .view
            .entities
            .iter()
            .filter(|entity| {
                entity.payloads().all(|(path, descriptor)| {
                    provided.contains(&bundle_object(path, &descriptor.sha256))
                })
            })
            .map(|entity| &validated.world.entities[&entity.id].model)
            .collect::<BTreeSet<_>>()
            .len();
        for (path, bytes) in &bundle.pages {
            io::atomic_write(&temporary.path().join(path), bytes.as_bytes())?;
        }
        // Empty worlds still have the same safe directory layout.
        fs::create_dir_all(temporary.path().join("chunks"))?;
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
            entity_count: manifest.entity_count,
            downloaded_entities: included.into_iter().collect(),
            version,
            model_count,
            included_models,
        })
    }
}

fn bundle_object(path: &str, sha: &str) -> String {
    if path.ends_with(".glb") {
        paths::object(sha)
    } else {
        format!(
            "{}{}",
            paths::OBJECTS,
            path.rsplit('/').next().expect("validated texture path")
        )
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
        if name == "objects" || name == "chunks" {
            let chunks = name == "chunks";
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
                if chunks {
                    let key = name
                        .strip_suffix(".json")
                        .context("unrelated file in chunks directory")?;
                    rou_core::spatial::parse_chunk(key)?;
                } else if let Some(hash) = name.strip_suffix(".glb") {
                    rou_core::documents::hash_shape(hash)?;
                } else {
                    ensure!(
                        "invalid_layout",
                        rou_core::documents::Texture::uri(name).is_ok(),
                        "unrelated file in objects directory"
                    );
                }
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
