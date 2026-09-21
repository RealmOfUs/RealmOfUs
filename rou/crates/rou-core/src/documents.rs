//! Types, field constraints and semantic validation for versioned documents and host responses.
//!
//! Document deserialization rejects unknown fields and runs each type's `validate_shape`.
//! Nested document shapes are also checked on read; manual struct construction does not
//! run these checks. Full cross-field/document constraints are enforced by `validate`
//! methods and [`super::world::World`].
//!
//! Optional strings may be omitted, but explicit `null` does not mean omission.
//! Read raw JSON bytes through [`json::parse`] to retain duplicate-key, encoding
//! and resource-budget checks.
use crate::{
    Result,
    error::require,
    formats, json, limits, paths,
    spatial::{Bounds, Profile},
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

// Keep the serde wire shape and public type in one declaration. Cross-document checks
// remain explicit methods, separate from the shape checks performed while parsing.
macro_rules! document {
    ($(#[$meta:meta])* $name:ident { $($(#[$attr:meta])* $field:ident: $ty:ty),* $(,)? }) => {
        $(#[$meta])*
        #[derive(Debug, Clone, PartialEq, serde::Serialize)]
        pub struct $name { $($(#[$attr])* pub $field: $ty),* }
        impl<'de> serde::Deserialize<'de> for $name {
            fn deserialize<D: serde::Deserializer<'de>>(reader: D) -> std::result::Result<Self, D::Error> {
                #[derive(serde::Deserialize)]
                #[serde(deny_unknown_fields)]
                struct Wire { $($(#[$attr])* $field: $ty),* }
                let wire = Wire::deserialize(reader)?;
                let value = Self { $($field: wire.$field),* };
                value.validate_shape().map_err(serde::de::Error::custom)?;
                Ok(value)
            }
        }
    };
}
pub(crate) use document;

/// Check for exactly 64 lowercase hexadecimal digits representing a SHA-256 digest.
///
/// # Errors
///
/// Return `invalid_hash` for invalid length or characters; does not compare against an actual payload.
pub fn hash_shape(value: &str) -> Result<()> {
    require(
        value.len() == 64
            && value
                .bytes()
                .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c)),
        "invalid_hash",
        "expected a lowercase SHA-256",
    )
}

/// Serde adapter for an optional string field that must be a string when present.
///
/// Use with `#[serde(default)]`: omission yields `None`, an explicit string yields `Some`.
///
/// # Errors
///
/// Return a deserializer error for explicit `null` or any non-string value.
pub fn optional_string<'de, D: serde::Deserializer<'de>>(
    reader: D,
) -> std::result::Result<Option<String>, D::Error> {
    Ok(Some(String::deserialize(reader)?))
}

fn optional_versions<'de, D: serde::Deserializer<'de>>(
    reader: D,
) -> std::result::Result<Option<BTreeMap<String, String>>, D::Error> {
    Ok(Some(BTreeMap::deserialize(reader)?))
}

/// Check the Unicode scalar value count of an optional display label without restricting its character set.
///
/// Both omission and an empty string are valid.
///
/// # Errors
///
/// Return `invalid_label` when the length exceeds [`limits::LABEL_CHARS`].
pub fn label(value: &Option<String>) -> Result<()> {
    require(
        value
            .as_ref()
            .is_none_or(|v| v.chars().count() <= limits::LABEL_CHARS),
        "invalid_label",
        "label exceeds character limit",
    )
}

document!(
    /// Root rules in `realm.json`: format, Realm ID and spatial profile.
    ///
    /// The entire document is immutable after initialization; [`super::world::lineage`]
    /// checks ancestor agreement. Contains no viewer start or viewing entry.
    Realm {
        /// Must equal [`formats::REALM`].
        format: String,
        /// Stable Realm ID, in a namespace separate from model and entity IDs.
        id: String,
        /// Spatial rules shared by every model and entity.
        profile: Profile,
    }
);
impl Realm {
    /// Check the root document format and Realm ID.
    ///
    /// # Errors
    ///
    /// Return `invalid_format` for an unsupported format or `invalid_id` for an invalid ID.
    /// A directly constructed profile needs a full check through [`Self::validate`].
    pub fn validate_shape(&self) -> Result<()> {
        require(
            self.format == formats::REALM,
            "invalid_format",
            "unsupported realm format",
        )?;
        paths::identity(&self.id)
    }
    /// Check the root document shape and all spatial constraints of its profile.
    ///
    /// # Errors
    ///
    /// Propagate shape and [`Profile::validate`] errors; does not check Git ancestor agreement.
    pub fn validate(&self) -> Result<()> {
        self.validate_shape()?;
        self.profile.validate()
    }
}

/// Blender model export target kind, serialized as a lowercase string.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EntryKind {
    /// A Blender object export target, serialized as `object`.
    Object,
    /// A Blender Collection export target, serialized as `collection`.
    Collection,
}

document!(
    /// Identifies a Blender object/collection to export; unrelated to the viewer start.
    Entry {
        /// Whether the target is one Object or a Collection.
        kind: EntryKind,
        /// Target name in Blender, containing 1-256 Unicode scalar values.
        name: String,
    }
);
impl Entry {
    /// Check that the export target name is nonempty and within the character budget.
    ///
    /// # Errors
    ///
    /// Return `invalid_entry` for an invalid name length; hosts check target existence in Blender.
    pub fn validate_shape(&self) -> Result<()> {
        require(
            !self.name.is_empty() && self.name.chars().count() <= limits::LABEL_CHARS,
            "invalid_entry",
            "entry name must contain 1\u{2013}256 characters",
        )
    }
}

document!(
    /// A model definition in `realm/models/<id>.json`, shareable by multiple entities.
    ///
    /// Source and dependency paths are repository-relative; hosts discover the dependency closure and declare it here.
    Model {
        /// Must equal [`formats::MODEL`].
        format: String,
        /// Stable ID in the model namespace; should match the filename.
        id: String,
        /// Path to the complete Blender source file; must end in `.blend`.
        source: String,
        /// The Object or Collection to export from the source file.
        entry: Entry,
        /// Declared dependency paths, excluding the source itself and containing no duplicates.
        dependencies: Vec<String>,
        /// Reserved export settings object; must currently be empty.
        settings: BTreeMap<String, Value>,
    }
);
impl Model {
    /// Check format, ID, source path, export target, dependency budgets and empty settings.
    ///
    /// # Errors
    ///
    /// Return errors for unsafe paths, a source without the `.blend` suffix, over-budget
    /// names/dependencies or nonempty settings. Does not check duplicate dependencies,
    /// cycles or files on disk.
    pub fn validate_shape(&self) -> Result<()> {
        require(
            self.format == formats::MODEL,
            "invalid_format",
            "unsupported model format",
        )?;
        paths::identity(&self.id)?;
        paths::safe(&self.source)?;
        require(
            self.source.ends_with(".blend"),
            "invalid_source",
            "source must end in .blend",
        )?;
        require(
            !self.source.starts_with(paths::RUNTIME),
            "invalid_source",
            "source cannot be a runtime output",
        )?;
        self.entry.validate_shape()?;
        require(
            self.dependencies.len() <= limits::DEPENDENCIES,
            "dependency_budget",
            "too many dependencies",
        )?;
        for dependency in &self.dependencies {
            paths::safe(dependency)?;
            require(
                !dependency.starts_with(paths::RUNTIME),
                "invalid_dependency",
                "dependency cannot be a runtime output",
            )?;
        }
        require(
            self.settings.is_empty(),
            "invalid_settings",
            "settings must be an empty object",
        )
    }
    /// After shape validation, reject duplicate dependencies and source self-dependencies.
    ///
    /// # Errors
    ///
    /// Propagate shape errors; duplicate/self dependencies return `invalid_dependency`.
    /// World validation checks cycles between model source files.
    pub fn validate(&self) -> Result<()> {
        self.validate_shape()?;
        let unique: BTreeSet<_> = self.dependencies.iter().collect();
        require(
            unique.len() == self.dependencies.len() && !unique.contains(&self.source),
            "invalid_dependency",
            "duplicate dependency or source depends on itself",
        )
    }
    /// Return the sorted, deduplicated union of the source and declared dependencies.
    ///
    /// Does not validate the model; useful for building the expected input key set.
    pub fn input_paths(&self) -> BTreeSet<String> {
        self.dependencies
            .iter()
            .cloned()
            .chain([self.source.clone()])
            .collect()
    }
}

document!(
    /// Entity placement in `realm/entities/<id>.json`, referencing a model and providing a world translation.
    Entity {
        /// Must equal [`formats::ENTITY`].
        format: String,
        /// Stable ID in the entity namespace; should match the filename.
        id: String,
        /// Referenced model ID; World validation checks that it exists.
        model: String,
        /// Y-up world position in X/Y/Z order, measured in integer millimetres.
        #[serde(deserialize_with = "json::vector")]
        position_mm: [i64; 3],
        /// Optional display label; may be omitted but cannot be `null` when present.
        #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "optional_string")]
        label: Option<String>,
    }
);
impl Entity {
    /// Check format, entity/model IDs, world millimetre coordinate budgets and the optional label.
    ///
    /// # Errors
    ///
    /// Propagate local field validation errors; does not check model existence or world membership.
    pub fn validate_shape(&self) -> Result<()> {
        require(
            self.format == formats::ENTITY,
            "invalid_format",
            "unsupported entity format",
        )?;
        paths::identity(&self.id)?;
        paths::identity(&self.model)?;
        crate::spatial::vector(&self.position_mm)?;
        label(&self.label)
    }
}

document!(
    /// Raw payload SHA-256 and byte count, without a path or any guarantee of local materialization.
    Descriptor {
        /// SHA-256 as 64 lowercase hexadecimal digits.
        sha256: String,
        /// Payload byte count, bounded by [`limits::FILE_BYTES`].
        #[serde(deserialize_with = "json::unsigned")]
        bytes: u64,
    }
);
impl Descriptor {
    /// Check SHA-256 shape and the file byte budget.
    ///
    /// # Errors
    ///
    /// Return `invalid_hash` or `file_budget`; does not read the payload.
    pub fn validate_shape(&self) -> Result<()> {
        hash_shape(&self.sha256)?;
        require(
            self.bytes <= limits::FILE_BYTES,
            "file_budget",
            "file byte budget exceeded",
        )
    }
    /// Compute a descriptor from arbitrary raw bytes without applying the file budget.
    ///
    /// Call [`Self::validate_shape`] before using the result as a valid document.
    pub fn of(data: &[u8]) -> Self {
        Self {
            sha256: json::digest(data),
            bytes: data.len() as u64,
        }
    }
    /// Validate the descriptor and compare its byte length and SHA-256 against the supplied payload.
    ///
    /// # Errors
    ///
    /// Propagate shape errors; return `length_mismatch` for a different length or `hash_mismatch` for a different digest.
    pub fn verify(&self, data: &[u8]) -> Result<()> {
        self.validate_shape()?;
        require(
            self.bytes == data.len() as u64,
            "length_mismatch",
            "payload length mismatch",
        )?;
        require(
            self.sha256 == json::digest(data),
            "hash_mismatch",
            "payload SHA-256 mismatch",
        )
    }
}

document!(
    /// A GLB reference: repository- or bundle-relative path, digest and byte count.
    ///
    /// Build/Manifest semantics define the exact path shape; this type only checks generic path safety and budgets.
    Asset {
        /// `runtime/<model>.glb` in builds, or `objects/<sha256>.glb` in browse bundles.
        path: String,
        /// The SHA-256 of raw GLB bytes, not a canonical JSON digest.
        sha256: String,
        /// Actual GLB byte count, in `20..=GLB_BYTES`.
        #[serde(deserialize_with = "json::unsigned")]
        bytes: u64,
    }
);
impl Asset {
    /// Check a safe relative path, SHA-256 shape and the GLB byte budget.
    ///
    /// # Errors
    ///
    /// Return path/digest errors or `asset_budget`; does not inspect GLB bytes or enforce build/bundle-specific paths.
    pub fn validate_shape(&self) -> Result<()> {
        paths::safe(&self.path)?;
        hash_shape(&self.sha256)?;
        require(
            (20..=limits::GLB_BYTES as u64).contains(&self.bytes),
            "asset_budget",
            "GLB byte budget exceeded",
        )
    }
    /// Extract the asset digest and byte count, dropping the path without revalidating.
    pub fn descriptor(&self) -> Descriptor {
        Descriptor {
            sha256: self.sha256.clone(),
            bytes: self.bytes,
        }
    }
}

/// Ordered map from repository-relative source/dependency paths to payload descriptors.
///
/// Keys must exactly match [`Model::input_paths`], including the source `.blend` itself.
/// Hosts obtain digests from actual payloads or declarations in standard LFS pointers.
pub type Inputs = BTreeMap<String, Descriptor>;

document!(
    /// Build record in `runtime/<model>.json`, used to check that a GLB matches its current inputs.
    ///
    /// Entity translations are excluded from the input digest, so model placement changes do not require a geometry rebuild.
    Build {
        /// Must equal [`formats::BUILD`].
        format: String,
        /// The model ID that owns this record.
        model: String,
        /// Descriptors of the source and all declared dependencies at build time.
        inputs: Inputs,
        /// Build input digest computed by [`input_digest`].
        input_digest: String,
        /// Output GLB, with path exactly `runtime/<model>.glb`.
        asset: Asset,
        /// GLB-local Y-up millimetre AABB; entity translation produces the world AABB.
        local_bounds_mm: Bounds,
        /// Optional tool-version information, excluded from the input digest; cannot be `null` when present.
        #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "optional_versions")]
        tool_versions: Option<BTreeMap<String, String>>,
    }
);
impl Build {
    /// Check record fields, input descriptors, the model-specific output path and a positive-thickness local AABB.
    ///
    /// # Errors
    ///
    /// Return errors for invalid format, ID, digest, dependency count/paths, GLB budget,
    /// output path or bounds. Does not determine whether current inputs have changed.
    pub fn validate_shape(&self) -> Result<()> {
        require(
            self.format == formats::BUILD,
            "invalid_format",
            "unsupported build format",
        )?;
        paths::identity(&self.model)?;
        hash_shape(&self.input_digest)?;
        require(
            self.inputs.len() <= limits::DEPENDENCIES + 1,
            "dependency_budget",
            "too many inputs",
        )?;
        for (path, input) in &self.inputs {
            paths::safe(path)?;
            input.validate_shape()?;
        }
        self.asset.validate_shape()?;
        require(
            self.asset.path == paths::asset(&self.model),
            "unsafe_path",
            "runtime path must match model ID",
        )?;
        self.local_bounds_mm.validate()
    }
    /// Check build-record freshness against the current profile, model definition and complete input descriptors.
    ///
    /// # Errors
    ///
    /// Propagate shape and input-digest errors. A model mismatch returns `invalid_reference`;
    /// different input descriptors or digests return `stale_build`. Hosts must verify
    /// actual GLB bytes separately.
    pub fn validate(&self, profile: &Profile, model: &Model, inputs: &Inputs) -> Result<()> {
        self.validate_shape()?;
        require(
            self.model == model.id,
            "invalid_reference",
            "build model mismatch",
        )?;
        require(
            &self.inputs == inputs,
            "stale_build",
            format!("stale input files: {}", model.id),
        )?;
        require(
            self.input_digest == input_digest(profile, model, inputs)?,
            "stale_build",
            format!("stale build: {}", model.id),
        )
    }
}

/// Compute the build input JCS SHA-256 over `{exporter, profile, model, inputs}`.
///
/// `exporter` is fixed to the helper format identifier. Actual tool versions, output
/// GLBs, build records and entity placements are excluded. Input path keys must exactly
/// match the model's source and declared dependency set.
///
/// # Errors
///
/// Propagate invalid profile/model/descriptor errors; a mismatched input key set returns `invalid_dependency`.
pub fn input_digest(profile: &Profile, model: &Model, inputs: &Inputs) -> Result<String> {
    profile.validate()?;
    model.validate()?;
    require(
        inputs.keys().cloned().collect::<BTreeSet<_>>() == model.input_paths(),
        "invalid_dependency",
        "inputs must exactly match source and dependencies",
    )?;
    for input in inputs.values() {
        input.validate_shape()?;
    }
    json::hash(&serde_json::json!({
        "exporter": formats::BLENDER_EXPORT,
        "profile": profile,
        "model": model,
        "inputs": inputs,
    }))
}

document!(
    /// An entity projection in a browse catalog; multiple entities may reference the same asset.
    PreviewEntity {
        /// Stable entity ID.
        id: String,
        /// Optional display label; omission produces no JSON field.
        #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "optional_string")]
        label: Option<String>,
        /// World translation in Y-up integer millimetres.
        #[serde(deserialize_with = "json::vector")]
        position_mm: [i64; 3],
        /// Owner chunk derived from position_mm, not the geometry coverage.
        #[serde(deserialize_with = "json::vector")]
        owner_chunk: [i64; 3],
        /// Nonnegative millimetre offset in the owner chunk, smaller than the chunk size on each axis.
        #[serde(deserialize_with = "json::vector")]
        offset_mm: [i64; 3],
        /// Translated world millimetre AABB with positive thickness on every axis.
        bounds_mm: Bounds,
        /// GLB reference; Manifest requires a content-addressed path.
        asset: Asset,
    }
);
impl PreviewEntity {
    /// Check entity ID, label, asset reference and a positive-thickness world AABB.
    ///
    /// # Errors
    ///
    /// Propagate local errors; [`Manifest::validate`] checks owner/offset agreement with the translation.
    pub fn validate_shape(&self) -> Result<()> {
        paths::identity(&self.id)?;
        label(&self.label)?;
        self.asset.validate_shape()?;
        self.bounds_mm.validate()
    }
}

document!(
    /// Complete browse-bundle metadata in `realm.json`, without authoring sources or viewing entries.
    ///
    /// `chunks` lists occupied chunks only; valid world chunks absent from the complete
    /// index are known to be empty. The catalog may reference GLBs not yet obtained;
    /// hosts check actual asset existence.
    Manifest {
        /// Must equal [`formats::PREVIEW`].
        format: String,
        /// The world's spatial rules.
        profile: Profile,
        /// The SHA-256 of the JCS-encoded profile.
        profile_hash: String,
        /// Must currently be true: metadata is complete, but assets need not be downloaded.
        index_complete: bool,
        /// Canonical chunk keys mapped to sorted entity ID lists, derived from world AABB coverage.
        chunks: BTreeMap<String, Vec<String>>,
        /// The complete entity list; IDs must be unique.
        entities: Vec<PreviewEntity>,
    }
);
impl Manifest {
    /// Check format, profile digest shape, complete-index flag and catalog count budgets.
    ///
    /// # Errors
    ///
    /// Return errors for invalid format/digest, an incomplete catalog or exceeded record/chunk
    /// budgets. [`Self::validate`] checks index contents and entity semantics.
    pub fn validate_shape(&self) -> Result<()> {
        require(
            self.format == formats::PREVIEW,
            "invalid_format",
            "unsupported preview format",
        )?;
        hash_shape(&self.profile_hash)?;
        require(
            self.index_complete,
            "incomplete_catalog",
            "preview requires a complete catalog",
        )?;
        require(
            self.entities.len() <= limits::WORLD_RECORDS
                && self.chunks.len() <= limits::WORLD_CHUNKS,
            "catalog_budget",
            "catalog record budget exceeded",
        )
    }
    /// Check the complete catalog and all derived spatial values without reading GLBs.
    ///
    /// Check the profile digest, unique entity IDs, owner chunks/offsets, content-addressed
    /// paths, shared asset lengths, world AABB coverage, and equality of the complete index
    /// with its recomputed value.
    ///
    /// # Errors
    ///
    /// Propagate shape, spatial or asset constraint errors. Inconsistent digests, placements
    /// or indexes return `hash_mismatch`, `invalid_placement` or `invalid_index`, respectively.
    pub fn validate(&self) -> Result<()> {
        self.validate_shape()?;
        self.profile.validate()?;
        require(
            self.profile_hash == json::hash(&self.profile)?,
            "hash_mismatch",
            "profile hash mismatch",
        )?;
        let mut ids = BTreeSet::new();
        let mut assets = BTreeMap::new();
        for entity in &self.entities {
            entity.validate_shape()?;
            require(
                ids.insert(&entity.id),
                "duplicate_id",
                "duplicate entity ID",
            )?;
            let (owner, offset) = self.profile.owner(entity.position_mm)?;
            require(
                owner == entity.owner_chunk && offset == entity.offset_mm,
                "invalid_placement",
                "noncanonical owner or offset",
            )?;
            require(
                entity.asset.path == paths::object(&entity.asset.sha256),
                "unsafe_path",
                "invalid content-addressed asset path",
            )?;
            if let Some(previous) = assets.insert(&entity.asset.sha256, entity.asset.bytes) {
                require(
                    previous == entity.asset.bytes,
                    "length_mismatch",
                    "shared asset length mismatch",
                )?;
            }
        }
        require(
            self.chunks == crate::spatial::build_index(&self.profile, &self.entities)?,
            "invalid_index",
            "spatial index mismatch",
        )
    }
}

document!(
    /// Git-exported `version.json` sidecar identifying a version and its locally trusted bridge result.
    ///
    /// This is `local-trusted` metadata, not an author signature or network authentication.
    /// [`Self::validate`] checks the catalog byte digest; it does not recover or validate
    /// the original Git tree from the browse catalog.
    Version {
        /// Must equal [`formats::GIT_VIEW`].
        format: String,
        /// Stable Realm ID, also used by the viewer to key local recent positions.
        realm_id: String,
        /// Git object algorithm: `sha1` or `sha256`.
        git_object_format: String,
        /// Full lowercase Git commit OID; a worktree draft may use an empty string.
        commit_oid: String,
        /// JCS digest of the in-memory View; shape validation does not prove its origin in the stated commit.
        view_root_sha256: String,
        /// SHA-256 of the raw catalog JSON bytes, including formatting differences.
        catalog_sha256: String,
        /// Currently only `local-trusted` is accepted.
        validation: String,
    }
);
impl Version {
    /// Check sidecar format, Realm ID, digest shapes, Git OID and validation level.
    ///
    /// # Errors
    ///
    /// Return domain errors for invalid fields; does not access Git or verify the declared view-root origin.
    pub fn validate_shape(&self) -> Result<()> {
        require(
            self.format == formats::GIT_VIEW,
            "invalid_format",
            "unsupported version format",
        )?;
        paths::identity(&self.realm_id)?;
        hash_shape(&self.view_root_sha256)?;
        hash_shape(&self.catalog_sha256)?;
        let length = match self.git_object_format.as_str() {
            "sha1" => 40,
            "sha256" => 64,
            _ => {
                return Err(crate::Error::new(
                    "invalid_git_oid",
                    "unsupported Git object format",
                ));
            }
        };
        require(
            self.commit_oid.is_empty()
                || (self.commit_oid.len() == length
                    && self
                        .commit_oid
                        .bytes()
                        .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))),
            "invalid_git_oid",
            "invalid commit OID",
        )?;
        require(
            self.validation == "local-trusted",
            "invalid_validation",
            "unsupported validation level",
        )
    }
    /// Bind the sidecar to the given catalog's raw bytes and fully validate that catalog.
    ///
    /// Does not recompute `view_root_sha256` or verify the Git tree/signature for `commit_oid`.
    /// The local Git host that creates the sidecar is responsible for establishing those relationships.
    ///
    /// # Errors
    ///
    /// Propagate sidecar, strict JSON and Manifest errors; a catalog byte-digest mismatch returns `hash_mismatch`.
    pub fn validate(&self, catalog: &[u8]) -> Result<()> {
        self.validate_shape()?;
        require(
            self.catalog_sha256 == json::digest(catalog),
            "hash_mismatch",
            "catalog hash mismatch",
        )?;
        json::parse::<Manifest>(catalog)?.validate()
    }
}

document!(
    /// One model result from the Blender helper, used by the build host to read and validate the actual GLB.
    ExportedModel {
        /// Exported model ID.
        id: String,
        /// Expected relative GLB path, exactly `runtime/<id>.glb`.
        path: String,
        /// Exported local Blender Z-up metre `[min, max]`; degenerate axes are allowed and later conservatively expanded to a millimetre AABB.
        bounds_blender_m: [[f64; 3]; 2],
        /// Helper-discovered dependency paths, all relative to the Realm root.
        dependencies: Vec<String>,
    }
);
impl ExportedModel {
    /// Check model ID, output path, finite non-reversed metre bounds and dependency-path budgets.
    ///
    /// # Errors
    ///
    /// Return errors for violations of these constraints. Zero-thickness bounds are allowed;
    /// integer-millimetre conversion later guarantees positive thickness. Does not check
    /// file existence, GLB contents or dependency-list agreement with the request.
    pub fn validate_shape(&self) -> Result<()> {
        paths::identity(&self.id)?;
        require(
            self.path == paths::asset(&self.id),
            "unsafe_path",
            "export path must match model ID",
        )?;
        require(
            self.bounds_blender_m
                .iter()
                .flatten()
                .all(|v| v.is_finite())
                && (0..3)
                    .all(|axis| self.bounds_blender_m[0][axis] <= self.bounds_blender_m[1][axis]),
            "invalid_bounds",
            "invalid exported model bounds",
        )?;
        require(
            self.dependencies.len() <= limits::DEPENDENCIES,
            "dependency_budget",
            "too many exported dependencies",
        )?;
        for path in &self.dependencies {
            paths::safe(path)?;
        }
        Ok(())
    }
}

document!(
    /// The response from one headless Blender export, possibly containing several models from one source.
    BlenderExport {
        /// Must equal [`formats::BLENDER_EXPORT`].
        format: String,
        /// Nonempty model result list with unique IDs.
        models: Vec<ExportedModel>,
        /// Tool-version map; `exporter` must equal the current helper format identifier.
        tool_versions: BTreeMap<String, String>,
    }
);
impl BlenderExport {
    /// Check helper format, a nonempty model list, unique IDs, each result and the exporter version.
    ///
    /// # Errors
    ///
    /// Return errors for unsupported format/version, an excessive model count, duplicate IDs or invalid model results.
    pub fn validate_shape(&self) -> Result<()> {
        require(
            self.format == formats::BLENDER_EXPORT,
            "invalid_format",
            "unsupported Blender export response",
        )?;
        require(
            !self.models.is_empty() && self.models.len() <= limits::WORLD_RECORDS,
            "world_budget",
            "invalid exported model count",
        )?;
        let mut ids = BTreeSet::new();
        for model in &self.models {
            model.validate_shape()?;
            require(
                ids.insert(&model.id),
                "duplicate_id",
                "duplicate exported model",
            )?;
        }
        require(
            self.tool_versions
                .get("exporter")
                .is_some_and(|v| v == formats::BLENDER_EXPORT),
            "invalid_format",
            "exporter version mismatch",
        )
    }
}
