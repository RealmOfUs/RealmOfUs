//! Pure rules for viewer coordinates, camera starts, entity loading sets and chunk maps.
//!
//! Cameras use Y-up metres and may lie outside world geometry; entity placements still
//! use integer millimetres. Derive the default start from the complete catalog; hosts
//! manage local recent positions and actual camera/asset operations.
use crate::{
    Result,
    documents::{Manifest, PreviewEntity},
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
/// Propagate [`Manifest::validate`] errors for an invalid catalog.
pub fn default_pose(manifest: &Manifest) -> Result<Pose> {
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
pub fn catalog(manifest: &Manifest) -> Result<Value> {
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
/// Default multiplier applied only to already resident middle-detail assets.
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
    /// IDs actually mounted by the host, not queued or in-flight entities.
    #[serde(default)]
    pub loaded: Vec<String>,
    /// Middle-radius multiplier, at least one; the product must fit `VIEW_RETENTION_METRES`.
    #[serde(default = "default_hysteresis")]
    pub hysteresis: f64,
}
/// A one-shot streaming query including a complete catalog.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Query {
    /// Complete metadata catalog, which may reference assets not yet materialized.
    pub manifest: Manifest,
    /// Current camera position in metres.
    pub position: Metres,
    /// Positive ordered near, middle and far radii in metres.
    pub radii: Metres,
    /// IDs actually mounted by the host.
    #[serde(default)]
    pub loaded: Vec<String>,
    /// Middle-radius multiplier; the product must fit `VIEW_RETENTION_METRES`.
    #[serde(default = "default_hysteresis")]
    pub hysteresis: f64,
}
/// Validate a catalog and select streaming actions using [`Catalog::query`].
///
/// # Errors
/// Propagate catalog, coordinate, radii, hysteresis and loaded-ID validation errors.
pub fn query(request: &Query) -> Result<Value> {
    Catalog::new(&request.manifest)?.query(&Residency {
        position: request.position,
        radii: request.radii,
        loaded: request.loaded.clone(),
        hysteresis: request.hysteresis,
    })
}

/// An immutable validated loading index, reusable for successive camera queries.
///
/// Construction validates untrusted metadata once. No mutable manifest escapes this type.
pub struct Catalog {
    profile: Profile,
    chunks: Vec<(Point, Vec<String>)>,
    centers: BTreeMap<String, Metres>,
}
impl Catalog {
    /// Validate a catalog and precompute chunk coordinates and entity centers.
    ///
    /// # Errors
    /// Propagate manifest validation errors without retaining partial state.
    pub fn new(manifest: &Manifest) -> Result<Self> {
        manifest.validate()?;
        Ok(Self {
            profile: manifest.profile.clone(),
            chunks: manifest
                .chunks
                .iter()
                .map(|(key, ids)| Ok((crate::spatial::parse_chunk(key)?, ids.clone())))
                .collect::<Result<_>>()?,
            centers: manifest
                .entities
                .iter()
                .map(|entity| {
                    (
                        entity.id.clone(),
                        std::array::from_fn(|a| {
                            (entity.bounds_mm.min[a] + entity.bounds_mm.max[a]) as f64 / 2000.0
                        }),
                    )
                })
                .collect(),
        })
    }
    /// Return disjoint `load`, `keep` and `unload` sets, and near/mid/far tier IDs.
    ///
    /// Distance is the minimum squared distance to any occupied chunk of an entity.
    /// Boundaries are inclusive; middle retention multiplies the radius by hysteresis
    /// and may extend beyond the far-box radius. Equal adjacent radii are allowed;
    /// the nearer tier wins a shared boundary. Duplicate loaded IDs are deduplicated.
    /// All arrays are ordered by entity AABB-center distance, with ID breaking ties.
    /// `impostor_hidden` equals `keep`; `impostor_visible` covers unmounted entities
    /// within the far radius (including IDs in `unload`). Hosts hide a loading impostor
    /// only after mounting the corresponding scene. `selected_chunks` counts occupied
    /// chunks within the middle radius. Missing GLBs still appear in `load`.
    ///
    /// # Errors
    /// Return errors for invalid coordinates/radii, a nonfinite or subunit hysteresis,
    /// an over-budget retention distance, or loaded IDs absent from this catalog.
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
            request.loaded.len() <= limits::WORLD_RECORDS
                && request
                    .loaded
                    .iter()
                    .all(|id| self.centers.contains_key(id)),
            "invalid_loaded",
            "loaded IDs must belong to the catalog and fit its budget",
        )?;
        let loaded: BTreeSet<_> = request.loaded.iter().collect();
        let p = &self.profile;
        let mut distances: BTreeMap<&String, f64> = BTreeMap::new();
        let mut selected = 0;
        for (chunk, ids) in &self.chunks {
            let lower: Metres = std::array::from_fn(|a| {
                (p.origin_mm[a] + chunk[a] * p.chunk_size_mm[a]) as f64 / 1000.0
            });
            let distance = (0..3)
                .map(|a| {
                    let nearest =
                        position[a].clamp(lower[a], lower[a] + p.chunk_size_mm[a] as f64 / 1000.0);
                    (position[a] - nearest).powi(2)
                })
                .sum::<f64>();
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
        let mut ordered: Vec<_> = self.centers.keys().collect();
        let distance = |id: &String| {
            (0..3)
                .map(|a| (self.centers[id][a] - position[a]).powi(2))
                .sum::<f64>()
        };
        ordered.sort_by(|a, b| distance(a).total_cmp(&distance(b)).then_with(|| a.cmp(b)));
        let (mut load, mut keep, mut unload) = (Vec::new(), Vec::new(), Vec::new());
        let (mut near, mut mid, mut far) = (Vec::new(), Vec::new(), Vec::new());
        let mut visible = Vec::new();
        for id in ordered {
            let d = distances[id];
            let retained = loaded.contains(id) && d <= retention.powi(2);
            if retained {
                keep.push(id);
            } else if loaded.contains(id) {
                unload.push(id);
            } else if d <= radii[1].powi(2) {
                load.push(id);
            }
            if d <= radii[0].powi(2) {
                near.push(id);
            } else if d <= radii[1].powi(2) || retained {
                mid.push(id);
            } else if d <= radii[2].powi(2) {
                far.push(id);
            }
            if !retained && d <= radii[2].powi(2) {
                visible.push(id);
            }
        }
        Ok(json!({
            "load": load, "keep": keep, "unload": unload,
            "impostor_hidden": keep, "impostor_visible": visible,
            "tiers": {"near": near, "mid": mid, "far": far},
            "selected_chunks": selected, "chunk": current, "chunk_key": chunk_key(current)
        }))
    }
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
