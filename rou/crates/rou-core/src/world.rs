//! Host-provided world snapshots, cross-document validation, browse projections and semantic diffs.
//!
//! All operations use in-memory values; this module does not read Git, download LFS
//! or open GLBs. [`World`] retains author metadata; hosts provide complete build
//! records separately to [`World::projections`].
//!
//! ```rust
//! use std::collections::BTreeMap;
//! use rou_core::{documents::Realm, formats, spatial::Profile, world::World};
//!
//! let world = World {
//!     realm: Realm { format: formats::REALM.into(), id: "empty_world".into(),
//!                    profile: Profile::default() },
//!     models: BTreeMap::new(), entities: BTreeMap::new(), inputs: BTreeMap::new(),
//! };
//! let view = world.projections(&BTreeMap::new())?;
//! assert!(view.manifest()?.entities.is_empty());
//! assert_eq!(view.root_hash()?.len(), 64);
//! # Ok::<(), rou_core::Error>(())
//! ```
use crate::{Error, Result, documents::*, error::require, formats, json, limits, paths, spatial};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

/// A complete snapshot of one Realm's author metadata and input descriptors.
///
/// May come from a commit, index or worktree; callers ensure snapshot consistency.
/// Deserialization checks nested document shapes, but cross-document references need explicit validation.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct World {
    /// Root rules that remain immutable after initialization.
    pub realm: Realm,
    /// Complete model definitions keyed by model ID; each key must equal the document ID.
    pub models: BTreeMap<String, Model>,
    /// Complete entity definitions keyed by entity ID; each key must equal the document ID.
    pub entities: BTreeMap<String, Entity>,
    /// Source/dependency descriptors keyed by model ID; full validation requires the same keys as models.
    pub inputs: BTreeMap<String, Inputs>,
}

impl World {
    /// Validate all metadata and every model's complete input descriptors.
    ///
    /// Payloads need not be materialized; hosts can provide descriptors from LFS pointers.
    ///
    /// # Errors
    ///
    /// Propagate [`Self::validate_metadata`] errors. Return the relevant dependency or
    /// field error for a mismatched input model set or invalid input paths/descriptors.
    pub fn validate(&self) -> Result<()> {
        self.validate_metadata()?;
        require(
            self.models.keys().eq(self.inputs.keys()),
            "invalid_dependency",
            "model inputs mismatch",
        )?;
        for (id, model) in &self.models {
            input_digest(&self.realm.profile, model, &self.inputs[id])?;
        }
        Ok(())
    }

    /// Validate complete metadata without requiring or reading cold-source descriptors in `inputs`.
    ///
    /// Check model/entity counts, ID/key agreement, model references, world placements,
    /// and cycles/depth in the registered source dependency graph. Ordinary dependencies
    /// not registered as model sources do not participate in source-graph traversal.
    ///
    /// # Errors
    ///
    /// Return errors for invalid documents, references or placements. Dependency cycles
    /// return `dependency_cycle`; excessive dependency depth returns `dependency_budget`.
    pub fn validate_metadata(&self) -> Result<()> {
        self.realm.validate()?;
        require(
            self.models.len() <= limits::WORLD_RECORDS
                && self.entities.len() <= limits::WORLD_RECORDS,
            "world_budget",
            "too many models or entities",
        )?;
        let mut graph: BTreeMap<&str, BTreeSet<&str>> = self
            .models
            .values()
            .map(|m| (m.source.as_str(), BTreeSet::new()))
            .collect();
        let sources: BTreeSet<_> = graph.keys().copied().collect();
        let mut entries = BTreeSet::new();
        for (id, model) in &self.models {
            require(
                id == &model.id,
                "invalid_id",
                "model filename must match ID",
            )?;
            model.validate()?;
            require(
                entries.insert((
                    &model.source,
                    model.entry.kind == EntryKind::Object,
                    &model.entry.name,
                )),
                "duplicate_entry",
                format!("models must not share a source entry: {}", model.entry.name),
            )?;
            graph
                .get_mut(model.source.as_str())
                .expect("source inserted above")
                .extend(
                    model
                        .dependencies
                        .iter()
                        .map(String::as_str)
                        .filter(|p| sources.contains(p)),
                );
        }
        for (id, entity) in &self.entities {
            entity.validate_shape()?;
            require(
                id == &entity.id,
                "invalid_id",
                "entity filename must match ID",
            )?;
            require(
                self.models.contains_key(&entity.model),
                "invalid_reference",
                format!("missing model: {}", entity.model),
            )?;
            self.realm.profile.owner(entity.position_mm)?;
        }
        let mut heights = BTreeMap::new();
        for source in graph.keys() {
            dependency_visit(source, &graph, &mut BTreeSet::new(), &mut heights, 0)?;
        }
        Ok(())
    }

    /// Compute a model's current build input digest without first validating the entire world.
    ///
    /// # Errors
    ///
    /// Return `invalid_reference` for an unknown model or `invalid_dependency` for missing
    /// input descriptors; other errors come from [`super::documents::input_digest`].
    pub fn input_digest(&self, id: &str) -> Result<String> {
        let model = self
            .models
            .get(id)
            .ok_or_else(|| Error::new("invalid_reference", "unknown model"))?;
        let inputs = self
            .inputs
            .get(id)
            .ok_or_else(|| Error::new("invalid_dependency", "missing model inputs"))?;
        input_digest(&self.realm.profile, model, inputs)
    }

    /// Validate all inputs and build records, then derive a complete browse projection.
    ///
    /// Require exactly one build record per model under the matching key. Translate local
    /// AABBs by entity placements to obtain world AABBs, then derive owner chunks, offsets
    /// and the complete coverage index. Asset paths still refer to runtime files here.
    ///
    /// # Errors
    ///
    /// Propagate world/build-record errors. Missing or extra records return `invalid_builds`;
    /// geometry outside the world or excessive chunk coverage also fails. Hosts must
    /// validate actual GLB bytes separately.
    pub fn projections(&self, records: &BTreeMap<String, Build>) -> Result<View> {
        self.validate()?;
        require(
            self.models.keys().eq(records.keys()),
            "invalid_builds",
            "missing or obsolete build records",
        )?;
        for (id, record) in records {
            record.validate(&self.realm.profile, &self.models[id], &self.inputs[id])?;
        }
        let mut entities = Vec::with_capacity(self.entities.len());
        for entity in self.entities.values() {
            let record = &records[&entity.model];
            let (owner_chunk, offset_mm) = self.realm.profile.owner(entity.position_mm)?;
            let bounds_mm = record.local_bounds_mm.translated(entity.position_mm)?;
            entities.push(PreviewEntity {
                id: entity.id.clone(),
                label: entity.label.clone(),
                position_mm: entity.position_mm,
                owner_chunk,
                offset_mm,
                bounds_mm,
                asset: record.asset.clone(),
            });
        }
        Ok(View {
            format: formats::VIEW.into(),
            realm_id: self.realm.id.clone(),
            profile: self.realm.profile.clone(),
            profile_hash: json::hash(&self.realm.profile)?,
            index_complete: true,
            chunks: spatial::build_index(&self.realm.profile, &entities)?,
            entities,
        })
    }
}

fn dependency_visit<'a>(
    source: &'a str,
    graph: &BTreeMap<&'a str, BTreeSet<&'a str>>,
    active: &mut BTreeSet<&'a str>,
    heights: &mut BTreeMap<&'a str, usize>,
    depth: usize,
) -> Result<usize> {
    require(
        depth <= limits::DEPENDENCY_DEPTH,
        "dependency_budget",
        "authoring dependency depth exceeded",
    )?;
    require(
        active.insert(source),
        "dependency_cycle",
        format!("cyclic source dependency: {source}"),
    )?;
    if let Some(height) = heights.get(source) {
        require(
            depth + height <= limits::DEPENDENCY_DEPTH,
            "dependency_budget",
            "authoring dependency depth exceeded",
        )?;
        active.remove(source);
        return Ok(*height);
    }
    let mut height = 0;
    for child in &graph[source] {
        height = height.max(dependency_visit(child, graph, active, heights, depth + 1)? + 1);
    }
    active.remove(source);
    heights.insert(source, height);
    Ok(height)
}

/// An in-memory derived view produced by [`World::projections`].
///
/// Used to generate browse bundles and stable view-root digests, not stored as a shared
/// index file. Direct construction/deserialization does not check that derived values
/// agree with the author world.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct View {
    /// Normally [`formats::VIEW`], set by the projection builder.
    pub format: String,
    /// The owning Realm's stable ID, included in the view-root digest.
    pub realm_id: String,
    /// The same spatial rules as the author world.
    pub profile: spatial::Profile,
    /// The JCS SHA-256 of the spatial profile.
    pub profile_hash: String,
    /// Whether the projection has a complete entity index; always true in the current production path.
    pub index_complete: bool,
    /// Occupied-chunk index derived from world AABBs.
    pub chunks: BTreeMap<String, Vec<String>>,
    /// Projections ordered by entity ID; assets still use runtime paths.
    pub entities: Vec<PreviewEntity>,
}
impl View {
    /// Replace asset paths with content-addressed paths and produce a validated browse catalog.
    ///
    /// Does not copy assets, emit a version sidecar or modify this View.
    ///
    /// # Errors
    ///
    /// Return [`Manifest::validate`] errors if the generated catalog fails validation.
    pub fn manifest(&self) -> Result<Manifest> {
        let mut entities = self.entities.clone();
        for entity in &mut entities {
            entity.asset.path = paths::object(&entity.asset.sha256);
        }
        let result = Manifest {
            format: formats::PREVIEW.into(),
            profile: self.profile.clone(),
            profile_hash: self.profile_hash.clone(),
            index_complete: self.index_complete,
            chunks: self.chunks.clone(),
            entities,
        };
        result.validate()?;
        Ok(result)
    }
    /// Compute the current View's JCS SHA-256 without first validating its contents.
    ///
    /// Callers should use a validated projection. The digest includes Realm ID, spatial
    /// rules, index and entities, but not the Git commit OID containing the view.
    ///
    /// # Errors
    ///
    /// Return a serialization error if canonical encoding fails.
    pub fn root_hash(&self) -> Result<String> {
        json::hash(self)
    }
}

/// Validate a record ID and require its path to equal `<directory><id>.json` exactly.
///
/// Callers supply a known directory prefix, such as [`paths::MODELS`] with its trailing
/// `/`. Does not normalize paths or independently validate the directory prefix.
///
/// # Errors
///
/// Return `invalid_id` for an invalid ID or a filename/ID mismatch.
pub fn record_id(path: &str, directory: &str, id: &str) -> Result<()> {
    paths::identity(id)?;
    require(
        path == format!("{directory}{id}.json"),
        "invalid_id",
        format!("record filename must match ID: {path}"),
    )
}

/// Validate that current root rules exactly match the host-selected ancestor's root rules.
///
/// Check every Realm field, including ID, format and profile. Does not access Git or
/// prove that `previous` is an ancestor; the Git host selects and traverses ancestors.
///
/// # Errors
///
/// Propagate Realm validation errors; differing root rules return `invalid_lineage`.
pub fn lineage(current: &Realm, previous: &Realm) -> Result<()> {
    current.validate()?;
    previous.validate()?;
    require(
        current == previous,
        "invalid_lineage",
        "immutable realm rules differ from ancestor",
    )
}

/// Compute the editing-session baseline digest for a Blender source file.
///
/// Validate only the source-scoped subset. Include models whose `source` matches exactly,
/// entities referencing those models, their input descriptors and Realm root rules in
/// the JCS digest. Unrelated metadata and cold descriptors need not be valid or provided. With no matching
/// models, still produce a baseline with empty sets; does not check source-file existence.
///
/// # Errors
///
/// Return validation errors for invalid metadata or missing/invalid input descriptors of related models.
pub fn session_fingerprint(world: &World, source: &str) -> Result<String> {
    let models: BTreeMap<_, _> = world
        .models
        .iter()
        .filter(|(_, m)| m.source == source)
        .collect();
    let entities: BTreeMap<_, _> = world
        .entities
        .iter()
        .filter(|(_, e)| models.contains_key(&e.model))
        .collect();
    let inputs: BTreeMap<_, _> = world
        .inputs
        .iter()
        .filter(|(id, _)| models.contains_key(id))
        .collect();
    // Editing one source must not require cold payloads from unrelated models.
    let scoped = World {
        realm: world.realm.clone(),
        models: models
            .iter()
            .map(|(k, v)| ((*k).clone(), (*v).clone()))
            .collect(),
        entities: entities
            .iter()
            .map(|(k, v)| ((*k).clone(), (*v).clone()))
            .collect(),
        inputs: inputs
            .iter()
            .map(|(k, v)| ((*k).clone(), (*v).clone()))
            .collect(),
    };
    scoped.validate()?;
    json::hash(
        &json!({"realm": world.realm, "models": models, "entities": entities, "inputs": inputs}),
    )
}

/// Compare two optional worlds and return an ID-sorted semantic diff as JSON; `None` means an empty world.
///
/// Return `realm_changes`, `added_entities`, `removed_entities`, `changed_entities`
/// and `model_changes`. Entity change categories are `placement`, `model`, `label`
/// and `geometry`. The last denotes a changed model input digest, not a direct
/// comparison of GLB geometry bytes.
///
/// # Errors
///
/// Return errors if either present world fails full validation or a model input digest cannot be computed.
pub fn diff(before: Option<&World>, after: Option<&World>) -> Result<Value> {
    let empty_models = BTreeMap::new();
    let empty_entities = BTreeMap::new();
    for world in [before, after].into_iter().flatten() {
        world.validate()?;
    }
    let old_models = before.map_or(&empty_models, |w| &w.models);
    let new_models = after.map_or(&empty_models, |w| &w.models);
    let old_entities = before.map_or(&empty_entities, |w| &w.entities);
    let new_entities = after.map_or(&empty_entities, |w| &w.entities);
    let mut model_changes = Vec::new();
    for id in old_models
        .keys()
        .chain(new_models.keys())
        .collect::<BTreeSet<_>>()
    {
        let old_digest = before
            .filter(|w| w.models.contains_key(id))
            .map(|w| w.input_digest(id))
            .transpose()?;
        let new_digest = after
            .filter(|w| w.models.contains_key(id))
            .map(|w| w.input_digest(id))
            .transpose()?;
        if old_digest != new_digest {
            let change = if old_digest.is_none() {
                "added"
            } else if new_digest.is_none() {
                "removed"
            } else {
                "modified"
            };
            model_changes.push(json!({
                "id": id,
                "change": change,
                "before": old_models.get(id),
                "after": new_models.get(id),
                "before_input_digest": old_digest,
                "after_input_digest": new_digest,
            }));
        }
    }
    let mut changed = Vec::new();
    for (id, old) in old_entities {
        let Some(new) = new_entities.get(id) else {
            continue;
        };
        let mut fields = Vec::new();
        if old.position_mm != new.position_mm {
            fields.push("placement");
        }
        if old.model != new.model {
            fields.push("model");
        }
        if old.label != new.label {
            fields.push("label");
        }
        if before
            .expect("entity came from before")
            .input_digest(&old.model)?
            != after
                .expect("entity came from after")
                .input_digest(&new.model)?
        {
            fields.push("geometry");
        }
        if !fields.is_empty() {
            changed.push(json!({"id": id, "changes": fields, "before": old, "after": new}));
        }
    }
    let old_realm = before.map(|w| &w.realm);
    let new_realm = after.map(|w| &w.realm);
    let old_value = serde_json::to_value(old_realm)?;
    let new_value = serde_json::to_value(new_realm)?;
    let realm_changes: Vec<_> = ["format", "id", "profile"]
        .into_iter()
        .filter(|k| old_value.get(k) != new_value.get(k))
        .collect();
    let added: Vec<_> = new_entities
        .keys()
        .filter(|id| !old_entities.contains_key(*id))
        .collect();
    let removed: Vec<_> = old_entities
        .keys()
        .filter(|id| !new_entities.contains_key(*id))
        .collect();
    Ok(json!({
        "realm_changes": realm_changes,
        "added_entities": added,
        "removed_entities": removed,
        "changed_entities": changed,
        "model_changes": model_changes,
    }))
}
