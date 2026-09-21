//! Validate metadata, ancestry, build freshness and available runtime payloads.
use crate::{
    Result, ensure,
    payload::MissingPayload,
    repository::Repository,
    snapshot::{Mode, Snapshot},
};
use bstr::{BString, ByteSlice};
use rou_core::{
    documents::Build,
    glb, json, lfs, limits, paths,
    world::{View, World},
};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Serialize)]
/// Metadata and byte-validation outcome, explicitly distinguishing missing inherited assets.
pub struct Validation {
    /// True after the operation has completed successfully.
    pub ok: bool,
    /// Stable ID from the immutable realm root document.
    pub realm_id: String,
    /// Number of entities in the complete catalog.
    pub entity_count: usize,
    /// Number of authoring models in the validated world.
    pub model_count: usize,
    /// Runtime paths whose actual payload bytes passed validation.
    pub validated_assets: Vec<String>,
    /// Runtime paths whose bytes were not checked or are inherited and unavailable.
    pub missing_assets: Vec<String>,
    /// Canonical hash of the complete derived view.
    pub view_root_sha256: String,
    /// True only when every required runtime payload was validated.
    pub byte_validation_complete: bool,
}

/// A validated world plus its retained snapshot for subsequent payload/view operations.
pub struct Validated {
    /// Captured input authority and payload resolver used for validation.
    pub snapshot: Snapshot,
    /// Validated authoring records and source descriptors.
    pub world: World,
    /// Fresh per-model build records.
    pub records: BTreeMap<String, Build>,
    /// Complete derived world projection.
    pub view: View,
    /// Serializable validation outcome.
    pub report: Validation,
}

impl Repository {
    /// Validate a snapshot, its ancestry and build records, optionally checking GLB bytes.
    /// Inherited missing payloads are reported as incomplete; new assets must be obtained and checked.
    /// Does not change the worktree or index and does not require the author writer lock.
    pub fn validate(&self, mode: Mode, verify_assets: bool) -> Result<Validated> {
        let snapshot = Snapshot::new(self, mode)?;
        ensure!(
            "unresolved_conflict",
            snapshot.conflicts.is_empty(),
            "unresolved Git conflicts; resolve inputs, build, then add outputs"
        );
        let world = snapshot.world(true)?;
        let records = snapshot.records(&world, false)?;
        let view = world.projections(&records)?;
        if snapshot.mode != Mode::Worktree {
            for input in world
                .inputs
                .values()
                .flat_map(|inputs| inputs.keys())
                .collect::<BTreeSet<_>>()
            {
                ensure!(
                    "lfs_required",
                    lfs::pointer(&snapshot.read(input.as_bytes(), limits::POINTER_BYTES)?)?
                        .is_some(),
                    "source must be tracked by Git LFS: {input}"
                );
            }
        }
        let actual: BTreeSet<BString> = snapshot
            .paths
            .iter()
            .filter(|p| p.starts_with(paths::RUNTIME.as_bytes()))
            .cloned()
            .collect();
        let expected: BTreeSet<BString> = world
            .models
            .keys()
            .flat_map(|id| {
                [
                    BString::from(paths::asset(id)),
                    BString::from(paths::build(id)),
                ]
            })
            .collect();
        ensure!(
            "stale_runtime",
            actual == expected,
            "missing or obsolete runtime files; run rou build (unexpected: {:?})",
            actual
                .difference(&expected)
                .map(|v| v.as_bstr())
                .collect::<Vec<_>>()
        );
        let mut previous = BTreeSet::new();
        if verify_assets {
            for parent in self.parents(snapshot.mode.commit())? {
                let parent = Snapshot::new(self, Mode::Commit(parent))?;
                for path in &parent.paths {
                    if path.starts_with(paths::RUNTIME.as_bytes()) && path.ends_with(b".json") {
                        if let Ok(record) = parent
                            .read(path, limits::JSON_BYTES)
                            .and_then(|b| Ok(json::parse::<Build>(&b)?))
                        {
                            previous.insert((record.asset.sha256, record.asset.bytes));
                        }
                    }
                }
            }
        }
        let mut validated_assets = Vec::new();
        let mut missing_assets = Vec::new();
        for record in records.values() {
            let asset = &record.asset;
            let descriptor = asset.descriptor();
            if snapshot.mode != Mode::Worktree {
                let pointer =
                    lfs::pointer(&snapshot.read(asset.path.as_bytes(), limits::POINTER_BYTES)?)?;
                ensure!(
                    "stale_runtime",
                    pointer.as_ref() == Some(&descriptor),
                    "runtime pointer differs from build record: {}",
                    asset.path
                );
            } else {
                ensure!(
                    "stale_runtime",
                    snapshot.descriptor(&asset.path)? == descriptor,
                    "runtime output differs from build record: {}",
                    asset.path
                );
            }
            if !verify_assets {
                missing_assets.push(asset.path.clone());
                continue;
            }
            let data = match snapshot.payload(&asset.path, &descriptor, false) {
                Ok(bytes) => bytes,
                Err(error) if error.downcast_ref::<MissingPayload>().is_some() => {
                    if previous.contains(&(asset.sha256.clone(), asset.bytes)) {
                        missing_assets.push(asset.path.clone());
                        continue;
                    }
                    snapshot.payload(&asset.path, &descriptor, true)?
                }
                Err(error) => return Err(error),
            };
            glb::precheck(&data)?;
            validated_assets.push(asset.path.clone());
        }
        snapshot.flush()?;
        let report = Validation {
            ok: true,
            realm_id: world.realm.id.clone(),
            entity_count: world.entities.len(),
            model_count: world.models.len(),
            view_root_sha256: view.root_hash()?,
            byte_validation_complete: missing_assets.is_empty(),
            validated_assets,
            missing_assets,
        };
        Ok(Validated {
            snapshot,
            world,
            records,
            view,
            report,
        })
    }
}
