//! Pure rules for viewer coordinates, camera starts, entity loading sets and chunk maps.
//!
//! Cameras use Y-up metres and may lie outside world geometry; entity placements still
//! use integer millimetres. Derive the default start from the complete catalog; hosts
//! manage local recent positions and actual camera/asset operations.
use crate::{
    Result,
    documents::{Descriptor, Manifest, Page, Preview, PreviewEntity, Texture},
    error::require,
    limits,
    spatial::{Point, Profile, chunk_key},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

/// X/Y/Z coordinates in metres; use [`coordinates`] to check finiteness and budgets.
pub type Metres = [f64; 3];
/// A camera position and look-at target, both in world metres.
///
/// Deserialization checks structure only; call [`pose`] for valid numeric ranges and distinct points.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Pose {
    /// Camera position in world metres.
    pub position: Metres,
    /// Camera look-at target; must differ from position.
    pub look_at: Metres,
}

/// Validate and return metre coordinates unchanged, without clipping to world bounds.
///
/// # Errors
///
/// Return `invalid_coordinate` for a nonfinite component or one exceeding [`limits::VIEW_COORDINATE_METRES`] in absolute value.
pub fn coordinates(position: Metres) -> Result<Metres> {
    require(
        position
            .iter()
            .all(|v| v.is_finite() && v.abs() <= limits::VIEW_COORDINATE_METRES),
        "invalid_coordinate",
        "coordinates must be finite metres within the precision budget",
    )?;
    Ok(position)
}
/// Divide integer millimetre components by 1000 to obtain metres, without checking the input budget.
pub fn metres(point: Point) -> Metres {
    point.map(|v| v as f64 / 1000.0)
}
/// Parse CLI/UI `x,y,z` coordinates in metres, allowing whitespace around each component.
///
/// # Errors
///
/// Return `invalid_coordinate` unless there are three parseable floating-point components accepted by [`coordinates`].
pub fn at(text: &str) -> Result<Metres> {
    let pieces: Vec<_> = text.split(',').collect();
    require(
        pieces.len() == 3,
        "invalid_coordinate",
        "enter x,y,z in metres",
    )?;
    let mut position = [0.0; 3];
    for (axis, value) in pieces.iter().enumerate() {
        position[axis] = value.trim().parse().map_err(|_| {
            crate::Error::new(
                "invalid_coordinate",
                "enter three numeric metre coordinates",
            )
        })?;
    }
    coordinates(position)
}
/// Validate finite, bounded, distinct camera/target coordinates and return the pose unchanged.
///
/// # Errors
///
/// Return `invalid_coordinate` for invalid or coincident points; does not check collisions or world bounds.
pub fn pose(pose: Pose) -> Result<Pose> {
    coordinates(pose.position)?;
    coordinates(pose.look_at)?;
    require(
        pose.position != pose.look_at,
        "invalid_coordinate",
        "look target must differ from position",
    )?;
    Ok(pose)
}
/// Aim at a supplied target, falling back to the negative Z axis for coincident points.
///
/// # Errors
/// Propagate [`pose`] coordinate errors, including a fallback outside the precision budget.
pub fn aim(mut request: Pose) -> Result<Pose> {
    if request.position == request.look_at {
        request.look_at[2] -= 1.0;
    }
    pose(request)
}

/// Validate the catalog and place the default start one union-AABB diagonal above its center.
///
/// Offset the camera along +Y and aim at the center. For an empty world, return
/// `[0,2,0]` looking at the origin. Does not change the viewing radii; a high overview
/// can show catalog boxes without loading full assets.
///
/// # Errors
///
/// Propagate [`Preview::validate`] errors for an invalid catalog.
pub fn default_pose(manifest: &Preview) -> Result<Pose> {
    manifest.validate()?;
    if manifest.entities.is_empty() {
        return Ok(Pose {
            position: [0., 2., 0.],
            look_at: [0., 0., 0.],
        });
    }
    let mut lower = [i64::MAX; 3];
    let mut upper = [i64::MIN; 3];
    for entity in &manifest.entities {
        for a in 0..3 {
            lower[a] = lower[a].min(entity.bounds_mm.min[a]);
            upper[a] = upper[a].max(entity.bounds_mm.max[a]);
        }
    }
    let center = std::array::from_fn(|a| (lower[a] + upper[a]) as f64 / 2000.0);
    let distance = (0..3)
        .map(|a| ((upper[a] - lower[a]) as f64 / 1000.0).powi(2))
        .sum::<f64>()
        .sqrt();
    Ok(Pose {
        position: [center[0], center[1] + distance, center[2]],
        look_at: center,
    })
}
/// Create an observation pose for a validated entity, looking at its world AABB center.
///
/// Use the AABB diagonal as the distance scale, with a 2 m minimum, and offset by
/// `(0.6,0.5,0.8)` times that scale. Does not validate the entity, handle obstacles or load assets.
pub fn focus(entity: &PreviewEntity) -> Pose {
    let center = std::array::from_fn(|a| {
        (entity.bounds_mm.min[a] + entity.bounds_mm.max[a]) as f64 / 2000.0
    });
    let distance = (0..3)
        .map(|a| ((entity.bounds_mm.max[a] - entity.bounds_mm.min[a]) as f64 / 1000.).powi(2))
        .sum::<f64>()
        .sqrt()
        .max(2.0);
    Pose {
        position: [
            center[0] + distance * 0.6,
            center[1] + distance * 0.5,
            center[2] + distance * 0.8,
        ],
        look_at: center,
    }
}
/// Compute a camera's chunk index by flooring against the grid origin and chunk sizes.
///
/// Cameras may be outside the world, so the result need not satisfy [`Profile::contains_chunk`].
///
/// # Errors
///
/// Return validation errors for an invalid profile or over-budget camera coordinates.
pub fn chunk_at(profile: &Profile, position: Metres) -> Result<Point> {
    profile.validate()?;
    coordinates(position)?;
    Ok(std::array::from_fn(|a| {
        ((position[a] * 1000.0 - profile.origin_mm[a] as f64) / profile.chunk_size_mm[a] as f64)
            .floor() as i64
    }))
}
/// Return the center of a specified in-world chunk in metres.
///
/// # Errors
///
/// Propagate profile errors; return `outside_world` for an index outside the half-open world range.
pub fn chunk_center(profile: &Profile, chunk: Point) -> Result<Metres> {
    profile.validate()?;
    require(
        (0..3).all(|a| chunk[a] >= profile.min_chunk[a] && chunk[a] < profile.max_chunk[a]),
        "outside_world",
        "chunk is outside this realm",
    )?;
    Ok(std::array::from_fn(|a| {
        (profile.origin_mm[a] as f64 + (chunk[a] as f64 + 0.5) * profile.chunk_size_mm[a] as f64)
            / 1000.0
    }))
}
/// Derive viewer initialization JSON from a complete catalog.
///
/// Return `known` (all world chunks, including empty ones), `entities` (metre positions,
/// metre bounds and focus poses keyed by ID), `start` (the default pose), and
/// `impostors` (all entity world-bound centers and sizes in metres).
/// Does not access GLBs or local recent positions.
///
/// # Errors
///
/// Propagate validation errors for an invalid catalog.
pub fn catalog(manifest: &Preview) -> Result<Value> {
    manifest.validate()?;
    let p = &manifest.profile;
    let mut known = BTreeMap::new();
    for x in p.min_chunk[0]..p.max_chunk[0] {
        for y in p.min_chunk[1]..p.max_chunk[1] {
            for z in p.min_chunk[2]..p.max_chunk[2] {
                let key = chunk_key([x, y, z]);
                known.insert(
                    key.clone(),
                    manifest.chunks.get(&key).cloned().unwrap_or_default(),
                );
            }
        }
    }
    let entities: BTreeMap<_, _> = manifest.entities.iter().map(|entity| {
        (&entity.id, json!({
            "position": metres(entity.position_mm),
            "bounds": {"min": metres(entity.bounds_mm.min), "max": metres(entity.bounds_mm.max)},
            "focus": focus(entity)
        }))
    }).collect();
    Ok(json!({
        "known": known, "entities": entities, "start": default_pose(manifest)?,
        "layers": {"min": p.min_chunk[1], "max": p.max_chunk[1] - 1},
        "impostors": manifest.entities.iter().map(|entity| json!({
            "id": entity.id,
            "center_m": std::array::from_fn::<_, 3, _>(|a|
                (entity.bounds_mm.min[a] + entity.bounds_mm.max[a]) as f64 / 2000.0),
            "size_m": std::array::from_fn::<_, 3, _>(|a|
                (entity.bounds_mm.max[a] - entity.bounds_mm.min[a]) as f64 / 1000.0)
        })).collect::<Vec<_>>()
    }))
}
/// Default near, middle and far radii, in metres; viewer settings, not profile data.
pub fn default_radii() -> Metres {
    [
        limits::VIEW_NEAR_METRES,
        limits::VIEW_MID_METRES,
        limits::VIEW_FAR_METRES,
    ]
}
/// Default multiplier for resident assets and the near tier of mounted full assets.
pub fn default_hysteresis() -> f64 {
    limits::VIEW_HYSTERESIS
}
/// Validate positive, ordered, finite near/middle/far radii within the camera budget.
///
/// # Errors
/// Return `invalid_radii` for invalid ranges; equal adjacent radii are allowed.
pub fn radii(value: Metres) -> Result<Metres> {
    require(
        value
            .iter()
            .all(|v| v.is_finite() && *v > 0.0 && *v <= limits::VIEW_COORDINATE_METRES)
            && value[0] <= value[1]
            && value[1] <= value[2],
        "invalid_radii",
        "radii must be positive ordered near,middle,far distances in metres",
    )?;
    Ok(value)
}
/// A query against an already validated catalog; no filesystem availability is implied.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Residency {
    /// Current camera position in metres, including outside-world overview positions.
    pub position: Metres,
    /// Near, middle and far distances to occupied chunk AABBs, in metres.
    pub radii: Metres,
    /// Entity IDs mapped to actually mounted asset SHAs, excluding queued/in-flight assets.
    #[serde(default)]
    pub loaded: BTreeMap<String, String>,
    /// Near/retention multiplier, at least one; middle product must fit `VIEW_RETENTION_METRES`.
    #[serde(default = "default_hysteresis")]
    pub hysteresis: f64,
}
/// A one-shot streaming query including a complete catalog.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Query {
    /// Complete metadata catalog, which may reference assets not yet materialized.
    pub manifest: Preview,
    /// Current camera position in metres.
    pub position: Metres,
    /// Positive ordered near, middle and far radii in metres.
    pub radii: Metres,
    /// Entity IDs mapped to actually mounted full or lod1 asset SHAs.
    #[serde(default)]
    pub loaded: BTreeMap<String, String>,
    /// Near/retention multiplier; middle product must fit `VIEW_RETENTION_METRES`.
    #[serde(default = "default_hysteresis")]
    pub hysteresis: f64,
}
/// Validate a catalog and select streaming actions using [`Catalog::query`].
///
/// # Errors
/// Propagate catalog, coordinate, radii, hysteresis and loaded-asset validation errors.
pub fn query(request: &Query) -> Result<Value> {
    Catalog::new(&request.manifest)?.query(&Residency {
        position: request.position,
        radii: request.radii,
        loaded: request.loaded.clone(),
        hysteresis: request.hysteresis,
    })
}

/// An incrementally validated loading index, reusable for successive camera queries.
///
/// Root/page loading validates untrusted metadata before mutation. No mutable manifest escapes this type.
pub struct Catalog {
    root: Manifest,
    root_chunks: Vec<(String, Point)>,
    pages: BTreeMap<String, Page>,
    profile: Profile,
    chunks: BTreeMap<String, (Point, BTreeSet<String>)>,
    centers: BTreeMap<String, Metres>,
    assets: BTreeMap<String, (String, String)>,
    entities: BTreeMap<String, PreviewEntity>,
    payloads: BTreeMap<String, Descriptor>,
    texture_sets: BTreeMap<String, Vec<Texture>>,
}
impl Catalog {
    /// Derive a host layout delta from a verified page.
    pub fn page_layout(&self, page: &Page) -> Result<Value> {
        page_layout(&self.profile, page)
    }
    /// Validate a root and start with no loaded pages or entities.
    pub fn load_root(root: &Manifest) -> Result<Self> {
        root.validate()?;
        Ok(Self {
            root: root.clone(),
            root_chunks: root
                .chunks
                .keys()
                .map(|key| Ok((key.clone(), crate::spatial::parse_chunk(key)?)))
                .collect::<Result<_>>()?,
            profile: root.profile.clone(),
            pages: BTreeMap::new(),
            chunks: BTreeMap::new(),
            centers: BTreeMap::new(),
            assets: BTreeMap::new(),
            entities: BTreeMap::new(),
            payloads: BTreeMap::new(),
            texture_sets: BTreeMap::new(),
        })
    }
    /// Build a fully loaded index from a complete in-memory preview.
    pub fn new(preview: &Preview) -> Result<Self> {
        let bundle = preview.bundle()?;
        let mut catalog = Self::load_root(&bundle.root)?;
        for (key, descriptor) in &bundle.root.chunks {
            catalog.load_page(key, bundle.pages[&descriptor.page].as_bytes())?;
        }
        Ok(catalog)
    }
    /// Return the root-only layout and a conservative overview of occupied chunk bounds.
    pub fn layout(&self) -> Result<Value> {
        let preview = Preview {
            format: crate::formats::PREVIEW.into(),
            profile: self.profile.clone(),
            profile_hash: self.root.profile_hash.clone(),
            index_complete: true,
            chunks: BTreeMap::new(),
            entities: Vec::new(),
        };
        let mut result = catalog(&preview)?;
        if !self.root.chunks.is_empty() {
            let mut lower = [f64::INFINITY; 3];
            let mut upper = [f64::NEG_INFINITY; 3];
            for key in self.root.chunks.keys() {
                let chunk = crate::spatial::parse_chunk(key)?;
                for a in 0..3 {
                    let lo = (self.profile.origin_mm[a] + chunk[a] * self.profile.chunk_size_mm[a])
                        as f64
                        / 1000.0;
                    lower[a] = lower[a].min(lo);
                    upper[a] = upper[a].max(lo + self.profile.chunk_size_mm[a] as f64 / 1000.0);
                }
            }
            let center: Metres = std::array::from_fn(|a| (lower[a] + upper[a]) / 2.0);
            let diagonal = (0..3)
                .map(|a| (upper[a] - lower[a]).powi(2))
                .sum::<f64>()
                .sqrt();
            result["start"] = json!(Pose {
                position: [center[0], center[1] + diagonal, center[2]],
                look_at: center
            });
        }
        result["pending_pages"] = json!(self.root.chunks.keys().collect::<Vec<_>>());
        Ok(result)
    }
    /// Verify a page and all currently decidable cross-page references before updating state.
    /// Re-loading the same root-bound page is idempotent; a new root creates a new index.
    pub fn load_page(&mut self, key: &str, bytes: &[u8]) -> Result<Page> {
        let page = self.root.load_page(key, bytes)?;
        if self.pages.contains_key(key) {
            return Ok(page);
        }
        let mut payloads = BTreeMap::new();
        let mut texture_sets = BTreeMap::new();
        for entity in &page.entities {
            require(
                !self.entities.contains_key(&entity.id),
                "duplicate_id",
                "entity occurs on multiple owner pages",
            )?;
            for (path, descriptor) in entity.payloads() {
                if let Some(previous) = payloads.get(path).or_else(|| self.payloads.get(path)) {
                    require(
                        previous == &descriptor,
                        "length_mismatch",
                        "shared payload descriptor mismatch",
                    )?;
                }
                payloads.insert(path.to_owned(), descriptor);
            }
            for (asset, textures) in [
                (&entity.asset, &entity.textures.asset),
                (&entity.lod1, &entity.textures.lod1),
            ] {
                if let Some(previous) = texture_sets
                    .get(&asset.sha256)
                    .or_else(|| self.texture_sets.get(&asset.sha256))
                {
                    require(
                        previous == textures,
                        "invalid_texture",
                        "shared GLB texture declarations differ",
                    )?;
                }
                texture_sets.insert(asset.sha256.clone(), textures.clone());
            }
            for chunk in self.profile.intersections(&entity.bounds_mm)? {
                let covered = chunk_key(chunk);
                require(
                    self.root.chunks.contains_key(&covered),
                    "invalid_index",
                    "root omits an occupied coverage page",
                )?;
                if covered != key
                    && let Some(loaded) = self.pages.get(&covered)
                {
                    require(
                        loaded
                            .foreign
                            .iter()
                            .any(|r| r.id == entity.id && r.owner == key),
                        "invalid_index",
                        "coverage page omits foreign entity",
                    )?;
                }
            }
        }
        let foreign: BTreeMap<_, _> = page
            .foreign
            .iter()
            .map(|r| (r.id.as_str(), r.owner.as_str()))
            .collect();
        if let Some((_, ids)) = self.chunks.get(key) {
            for id in ids {
                let entity = &self.entities[id];
                require(
                    foreign
                        .get(id.as_str())
                        .is_some_and(|owner| *owner == chunk_key(entity.owner_chunk)),
                    "invalid_index",
                    "page omits an already loaded foreign entity",
                )?;
            }
        }
        for reference in &page.foreign {
            if self.pages.contains_key(&reference.owner) {
                require(
                    self.entities.get(&reference.id).is_some_and(|entity| {
                        chunk_key(entity.owner_chunk) == reference.owner
                            && self
                                .chunks
                                .get(key)
                                .is_some_and(|(_, ids)| ids.contains(&reference.id))
                    }),
                    "invalid_index",
                    "foreign reference differs from its owner page",
                )?;
            }
        }
        let owned: BTreeMap<_, _> = page
            .entities
            .iter()
            .map(|entity| (entity.id.as_str(), entity))
            .collect();
        for (covered, loaded) in &self.pages {
            for reference in loaded.foreign.iter().filter(|r| r.owner == key) {
                let entity = owned.get(reference.id.as_str()).ok_or_else(|| {
                    crate::Error::new("invalid_index", "foreign entity absent from owner page")
                })?;
                require(
                    self.profile
                        .intersections(&entity.bounds_mm)?
                        .into_iter()
                        .any(|chunk| chunk_key(chunk) == *covered),
                    "invalid_index",
                    "foreign entity does not intersect the page",
                )?;
            }
        }
        for entity in &page.entities {
            self.centers.insert(
                entity.id.clone(),
                std::array::from_fn(|a| {
                    (entity.bounds_mm.min[a] + entity.bounds_mm.max[a]) as f64 / 2000.0
                }),
            );
            self.assets.insert(
                entity.id.clone(),
                (entity.asset.sha256.clone(), entity.lod1.sha256.clone()),
            );
            for chunk in self.profile.intersections(&entity.bounds_mm)? {
                self.chunks
                    .entry(chunk_key(chunk))
                    .or_insert_with(|| (chunk, BTreeSet::new()))
                    .1
                    .insert(entity.id.clone());
            }
            self.entities.insert(entity.id.clone(), entity.clone());
        }
        self.payloads.extend(payloads);
        self.texture_sets.extend(texture_sets);
        self.pages.insert(key.to_owned(), page.clone());
        Ok(page)
    }
    /// Return disjoint `load`, `keep` and `unload` sets, and near/mid/far tier IDs.
    ///
    /// Distance is the minimum squared distance to any occupied chunk of an entity.
    /// Boundaries are inclusive. Mounted full assets keep the near tier through near
    /// radius times hysteresis. Middle retention uses the same multiplier and may
    /// extend beyond the far-box radius. The nearer tier wins a shared boundary.
    /// All arrays are ordered by entity AABB-center distance, with ID breaking ties.
    /// `load` includes resident scenes whose target asset differs; the target itself
    /// is implied by tier membership: near selects the full asset, otherwise lod1.
    /// `impostor_hidden` includes retained scenes awaiting replacement, while
    /// `impostor_visible` covers unmounted entities within the far radius, including
    /// IDs in `unload`. Hosts requery after failure-driven detachment and hide a box
    /// immediately after mounting. `selected_chunks` counts occupied chunks within
    /// the middle radius. Missing GLBs still appear in `load`.
    ///
    /// # Errors
    /// Return errors for invalid coordinates/radii, a nonfinite or subunit hysteresis,
    /// an over-budget retention distance, or loaded ID/SHA pairs absent from this catalog.
    pub fn query(&self, request: &Residency) -> Result<Value> {
        let position = request.position;
        let current = chunk_at(&self.profile, position)?;
        let radii = radii(request.radii)?;
        let retention = radii[1] * request.hysteresis;
        require(
            request.hysteresis.is_finite()
                && request.hysteresis >= 1.0
                && retention <= limits::VIEW_RETENTION_METRES,
            "invalid_hysteresis",
            "hysteresis must be finite, at least one and within the distance budget",
        )?;
        require(
            request.loaded.iter().all(|(id, sha)| {
                self.assets
                    .get(id)
                    .is_some_and(|(full, mid)| sha == full || sha == mid)
            }),
            "invalid_loaded",
            "loaded ID/SHA pairs must match catalog assets and fit its budget",
        )?;
        let p = &self.profile;
        let mut distances: BTreeMap<&String, f64> = BTreeMap::new();
        let mut selected = 0;
        let horizon = radii[2].max(retention).powi(2);
        let mut missing_pages = BTreeSet::new();
        for (key, chunk) in &self.root_chunks {
            if chunk_distance(p, *chunk, position) <= horizon {
                if let Some(page) = self.pages.get(key) {
                    for reference in &page.foreign {
                        if !self.pages.contains_key(&reference.owner) {
                            missing_pages.insert(reference.owner.clone());
                        }
                    }
                } else {
                    missing_pages.insert(key.clone());
                }
            }
        }
        for (chunk, ids) in self.chunks.values() {
            let distance = chunk_distance(p, *chunk, position);
            if distance > horizon {
                continue;
            }
            if distance <= radii[1].powi(2) {
                selected += 1;
            }
            for id in ids {
                distances
                    .entry(id)
                    .and_modify(|d| *d = d.min(distance))
                    .or_insert(distance);
            }
        }
        for id in request.loaded.keys() {
            distances.entry(id).or_insert(f64::INFINITY);
        }
        let mut ordered: Vec<_> = distances
            .into_iter()
            .map(|(id, chunk_distance)| {
                let center = self.centers[id];
                let center_distance = (0..3)
                    .map(|a| (center[a] - position[a]).powi(2))
                    .sum::<f64>();
                (id, chunk_distance, center_distance)
            })
            .collect();
        ordered.sort_by(|a, b| a.2.total_cmp(&b.2).then_with(|| a.0.cmp(b.0)));
        let (mut load, mut keep, mut unload) = (Vec::new(), Vec::new(), Vec::new());
        let (mut near, mut mid, mut far) = (Vec::new(), Vec::new(), Vec::new());
        let mut visible = Vec::new();
        let mut hidden = Vec::new();
        for (id, d, _) in ordered {
            let mounted = request.loaded.get(id);
            let (full, lod1) = &self.assets[id];
            // Equal SHAs are identical bytes: treat a mounted copy as full for
            // near hysteresis, and never schedule work solely to change its tier.
            let mounted_full = if full == lod1 {
                mounted.is_some()
            } else {
                mounted == Some(full)
            };
            let near_radius = if mounted_full {
                radii[0] * request.hysteresis
            } else {
                radii[0]
            };
            let is_near = d <= near_radius.powi(2);
            let target = if is_near { full } else { lod1 };
            let retained = mounted.is_some() && d <= retention.powi(2);
            if retained || d <= radii[1].powi(2) {
                if mounted == Some(target) {
                    keep.push(id);
                } else {
                    load.push(id);
                }
            } else if mounted.is_some() {
                unload.push(id);
            }
            if is_near {
                near.push(id);
            } else if d <= radii[1].powi(2) || retained {
                mid.push(id);
            } else if d <= radii[2].powi(2) {
                far.push(id);
            }
            if retained {
                hidden.push(id);
            } else if d <= radii[2].powi(2) {
                visible.push(id);
            }
        }
        Ok(json!({
            "load": load, "keep": keep, "unload": unload,
            "missing_pages": missing_pages,
            "impostor_hidden": hidden, "impostor_visible": visible,
            "tiers": {"near": near, "mid": mid, "far": far},
            "selected_chunks": selected, "chunk": current, "chunk_key": chunk_key(current)
        }))
    }
}
fn chunk_distance(profile: &Profile, chunk: Point, position: Metres) -> f64 {
    (0..3)
        .map(|a| {
            let lower =
                (profile.origin_mm[a] + chunk[a] * profile.chunk_size_mm[a]) as f64 / 1000.0;
            let nearest =
                position[a].clamp(lower, lower + profile.chunk_size_mm[a] as f64 / 1000.0);
            (position[a] - nearest).powi(2)
        })
        .sum()
}

/// Return only the entity and coverage additions for a newly loaded page.
pub fn page_layout(profile: &Profile, page: &Page) -> Result<Value> {
    let entities: BTreeMap<_, _> = page.entities.iter().map(|entity| {
        (&entity.id, json!({"position": metres(entity.position_mm),
            "bounds": {"min": metres(entity.bounds_mm.min), "max": metres(entity.bounds_mm.max)}, "focus": focus(entity)}))
    }).collect();
    let index = crate::spatial::build_index(profile, &page.entities)?;
    Ok(
        json!({"entities": entities, "known": index, "impostors": page.entities.iter().map(|entity| json!({
        "id": entity.id,
        "center_m": std::array::from_fn::<_, 3, _>(|a| (entity.bounds_mm.min[a] + entity.bounds_mm.max[a]) as f64 / 2000.0),
        "size_m": std::array::from_fn::<_, 3, _>(|a| (entity.bounds_mm.max[a] - entity.bounds_mm.min[a]) as f64 / 1000.0)
    })).collect::<Vec<_>>() }),
    )
}

/// A chunk-map layout request, fully checked by [`map`].
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MapRequest {
    /// Finite-world spatial rules used to clip the map window.
    pub profile: Profile,
    /// Current camera coordinates in world metres.
    pub position: Metres,
    /// Optional Y chunk index, not metres; defaults to the camera's Y layer and is clamped to the world.
    pub layer: Option<i64>,
}
/// Generate an X/Z map slice of at most 16 x 16 chunks near the camera.
///
/// Clamp the explicit layer or camera Y chunk index to the world; also keep the X/Z
/// window inside world bounds. Return `current`, `layer`, `nx`, `nz` and `cells`.
/// Each cell has local x/z, chunk, key and a center position in metres.
/// This provides a grid layout only, without determining occupancy.
///
/// # Errors
///
/// Return validation errors for an invalid profile or camera coordinates.
pub fn map(request: &MapRequest) -> Result<Value> {
    let p = &request.profile;
    let current = chunk_at(p, request.position)?;
    let nx = 16.min(p.max_chunk[0] - p.min_chunk[0]);
    let nz = 16.min(p.max_chunk[2] - p.min_chunk[2]);
    let x = (current[0] - nx / 2).clamp(p.min_chunk[0], p.max_chunk[0] - nx);
    let z = (current[2] - nz / 2).clamp(p.min_chunk[2], p.max_chunk[2] - nz);
    let y = request
        .layer
        .unwrap_or(current[1])
        .clamp(p.min_chunk[1], p.max_chunk[1] - 1);
    let mut cells = Vec::new();
    for dx in 0..nx {
        for dz in 0..nz {
            let chunk = [x + dx, y, z + dz];
            cells.push(json!({
                "x": dx,
                "z": dz,
                "chunk": chunk,
                "key": chunk_key(chunk),
                "position": chunk_center(p,chunk)?,
            }));
        }
    }
    Ok(json!({"current":current,"layer":y,"nx":nx,"nz":nz,"cells":cells}))
}
