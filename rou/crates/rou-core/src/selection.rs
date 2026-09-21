//! Resolve author entity/chunk selections into sparse directories, source payloads and runtime asset paths.
//!
//! Produces a deterministic IO plan only; does not run sparse-checkout, download, delete or write files.
use crate::{
    Result,
    error::require,
    paths, spatial,
    world::{View, World},
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// A set of unexpanded selectors, defaulting to an empty selection.
///
/// Deserialization rejects unknown fields but does not call [`Self::validate`].
/// The command layer decides whether an empty CLI selection is allowed.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Selection {
    #[serde(default)]
    /// Select all entities; mutually exclusive with chunks and entities.
    pub all: bool,
    #[serde(default)]
    /// Canonical single-chunk or inclusive chunk-range strings, expanded during resolution.
    pub chunks: Vec<String>,
    #[serde(default)]
    /// Explicit entity IDs that can be combined with chunk selections.
    pub entities: Vec<String>,
}

/// A resolved, sorted and deduplicated selection for a Git/LFS host to execute.
///
/// Directories determine the worktree cone; exact paths determine which payloads to materialize.
#[derive(Debug, Clone, Serialize)]
pub struct Resolved {
    /// Expanded explicit chunk selectors; excludes chunks only implied by entity selections or all.
    pub chunks: Vec<String>,
    /// Always includes BASE_CONE, plus parent directories of selected input files.
    pub directories: Vec<String>,
    /// Selected model source files and declared dependencies, excluding runtime GLBs.
    pub paths: Vec<String>,
    /// The selected models' `runtime/<model>.glb` paths.
    pub assets: Vec<String>,
    /// The union of explicit entity IDs, chunk hits or all entities.
    pub entity_ids: Vec<String>,
}

impl Selection {
    /// Check exclusivity of all versus explicit selectors, and validate explicit entity ID syntax.
    ///
    /// Does not parse chunk strings or check that entities exist.
    ///
    /// # Errors
    ///
    /// Return `invalid_selection` for all mixed with explicit selectors, or `invalid_id` for invalid IDs.
    pub fn validate(&self) -> Result<()> {
        require(
            !self.all || (self.chunks.is_empty() && self.entities.is_empty()),
            "invalid_selection",
            "--all excludes individual selectors",
        )?;
        for id in &self.entities {
            paths::identity(id)?;
        }
        Ok(())
    }
    /// Return whether all is unset and there are no entity/chunk selectors, without validation.
    pub fn is_empty(&self) -> bool {
        !self.all && self.chunks.is_empty() && self.entities.is_empty()
    }

    /// Resolve selectors into deterministic directory, source and asset sets.
    ///
    /// Fully validate `world`; `view` must be a valid projection of the same world and is
    /// neither rebuilt nor fully validated here. Union explicit entities with chunk-index hits
    /// and merge shared models' declared dependencies. An empty selection retains the base cone.
    ///
    /// # Errors
    ///
    /// Propagate world/selector validation errors. Return `invalid_selection` for a missing
    /// explicit entity, or `invalid_reference` when the index refers to a missing entity.
    pub fn resolve(&self, world: &World, view: &View) -> Result<Resolved> {
        self.validate()?;
        world.validate()?;
        let mut selected: BTreeSet<String> = if self.all {
            world.entities.keys().cloned().collect()
        } else {
            self.entities.iter().cloned().collect()
        };
        for id in &selected {
            require(
                world.entities.contains_key(id),
                "invalid_selection",
                format!("selected entity does not exist: {id}"),
            )?;
        }
        let chunks = world.realm.profile.range(&self.chunks)?;
        for chunk in &chunks {
            if let Some(ids) = view.chunks.get(&spatial::chunk_key(*chunk)) {
                selected.extend(ids.iter().cloned());
            }
        }
        let mut inputs = BTreeSet::new();
        let mut assets = BTreeSet::new();
        for id in &selected {
            let entity = world.entities.get(id).ok_or_else(|| {
                crate::Error::new(
                    "invalid_reference",
                    "selected index contains unknown entity",
                )
            })?;
            let model = &world.models[&entity.model];
            inputs.extend(model.input_paths());
            assets.insert(paths::asset(&model.id));
        }
        let mut directories: BTreeSet<String> =
            paths::BASE_CONE.iter().map(|p| (*p).into()).collect();
        for path in &inputs {
            if let Some((directory, _)) = path.rsplit_once('/') {
                directories.insert(directory.into());
            }
        }
        Ok(Resolved {
            chunks: chunks.into_iter().map(spatial::chunk_key).collect(),
            directories: directories.into_iter().collect(),
            paths: inputs.into_iter().collect(),
            assets: assets.into_iter().collect(),
            entity_ids: selected.into_iter().collect(),
        })
    }
}
