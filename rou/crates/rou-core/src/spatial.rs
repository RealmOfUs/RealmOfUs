//! Integer millimetres, half-open AABBs and a bounded Cartesian chunk grid.
//!
//! [`Point`] units depend on context: world coordinates/offsets use millimetres,
//! while chunk coordinates are integer indices. Euclidean division keeps offsets
//! nonnegative for negative coordinates. Geometry must have positive thickness on every axis.
use crate::{
    Error, Result,
    documents::{PreviewEntity, document},
    error::require,
    formats, json, limits,
};
use serde::{Deserialize, de};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

/// Three integers in X/Y/Z order, representing millimetres or chunk indices as documented by the caller.
pub type Point = [i64; 3];
/// Check that every component is bounded in absolute value by [`limits::COORDINATE_MM`].
///
/// # Errors
///
/// Return `invalid_coordinate` for an over-budget component; does not check world bounds or chunk size.
pub fn vector(value: &Point) -> Result<()> {
    require(
        value
            .iter()
            .all(|v| (-limits::COORDINATE_MM..=limits::COORDINATE_MM).contains(v)),
        "invalid_coordinate",
        "coordinate exceeds precision bound",
    )
}

fn quantization<'de, D: de::Deserializer<'de>>(reader: D) -> std::result::Result<i64, D::Error> {
    json::integer(&Value::deserialize(reader)?, 1, 1).map_err(de::Error::custom)
}

document!(
    /// Spatial conventions for a finite world; deserialization calls only [`Self::validate_shape`].
    ///
    /// Call [`Self::validate`] before full use. Defaults to right-handed Y-up with origin
    /// `[0,0,0]`, 32 m cubic chunks and chunk bounds `[-4,-2,-4]..[4,2,4)`,
    /// corresponding to a 256 x 128 x 256 m world.
    Profile {
        /// Fixed spatial profile identifier; see [`formats::PROFILE`].
        id: String,
        /// External metre unit identifier, fixed to `meter`.
        units: String,
        /// World up axis, fixed to `Y`.
        up: String,
        /// Coordinate system handedness, fixed to `right`.
        handedness: String,
        /// Placement quantization step, fixed to 1 mm.
        #[serde(deserialize_with = "quantization")]
        placement_unit_mm: i64,
        /// Grid origin in world millimetres.
        #[serde(deserialize_with = "json::vector")]
        origin_mm: Point,
        /// Chunk edge lengths in millimetres, each in `1..=64000`.
        #[serde(deserialize_with = "json::vector")]
        chunk_size_mm: Point,
        /// Inclusive minimum chunk index on each axis.
        #[serde(deserialize_with = "json::vector")]
        min_chunk: Point,
        /// Exclusive upper chunk index on each axis.
        #[serde(deserialize_with = "json::vector")]
        max_chunk: Point,
    }
);

impl Default for Profile {
    fn default() -> Self {
        Self {
            id: formats::PROFILE.into(),
            units: "meter".into(),
            up: "Y".into(),
            handedness: "right".into(),
            placement_unit_mm: 1,
            origin_mm: [0; 3],
            chunk_size_mm: [32000; 3],
            min_chunk: [-4, -2, -4],
            max_chunk: [4, 2, 4],
        }
    }
}

impl Profile {
    /// Check fixed identifiers, units, coordinate budgets and chunk sizes, but not total world volume.
    ///
    /// # Errors
    ///
    /// Return `invalid_profile` for invalid fixed values or chunk sizes, or `invalid_coordinate` for over-budget components.
    pub fn validate_shape(&self) -> Result<()> {
        require(
            self.id == formats::PROFILE
                && self.units == "meter"
                && self.up == "Y"
                && self.handedness == "right"
                && self.placement_unit_mm == 1,
            "invalid_profile",
            "unsupported profile constants",
        )?;
        for point in [
            &self.origin_mm,
            &self.chunk_size_mm,
            &self.min_chunk,
            &self.max_chunk,
        ] {
            vector(point)?;
        }
        require(
            self.chunk_size_mm
                .iter()
                .all(|v| (1..=limits::CHUNK_SIZE_MM).contains(v)),
            "invalid_profile",
            "invalid chunk size",
        )
    }
    /// Check all profile constraints, including half-open world bounds, endpoints and total chunk count.
    ///
    /// # Errors
    ///
    /// Propagate shape errors. Return `invalid_profile` for an empty/reversed world or
    /// millimetre endpoints outside the budget, and `spatial_budget` when the total chunk
    /// count exceeds [`limits::WORLD_CHUNKS`].
    pub fn validate(&self) -> Result<()> {
        self.validate_shape()?;
        let mut count = 1_i128;
        for axis in 0..3 {
            require(
                self.min_chunk[axis] < self.max_chunk[axis],
                "invalid_profile",
                "empty or inverted world",
            )?;
            count *= i128::from(self.max_chunk[axis] - self.min_chunk[axis]);
            for bound in [self.min_chunk, self.max_chunk] {
                let coordinate = self.origin_mm[axis] + bound[axis] * self.chunk_size_mm[axis];
                require(
                    (-limits::COORDINATE_MM..=limits::COORDINATE_MM).contains(&coordinate),
                    "invalid_profile",
                    "world exceeds precision bound",
                )?;
            }
        }
        require(
            count <= limits::WORLD_CHUNKS as i128,
            "spatial_budget",
            "too many world chunks",
        )
    }
    /// Check whether every axis of a chunk index is within `[min_chunk, max_chunk)`.
    ///
    /// Does not validate the profile; callers must have called [`Self::validate`].
    pub fn contains_chunk(&self, chunk: Point) -> bool {
        (0..3).all(|axis| self.min_chunk[axis] <= chunk[axis] && chunk[axis] < self.max_chunk[axis])
    }
    /// Return `(owner_chunk, offset_mm)` for a world position in millimetres.
    ///
    /// Offsets are within `[0, chunk_size_mm)` on each axis, including for negative world coordinates.
    ///
    /// # Errors
    ///
    /// Propagate profile/coordinate errors; return `outside_world` for a position outside the world or on its upper bound.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use rou_core::spatial::Profile;
    /// let profile = Profile::default();
    /// assert_eq!(profile.owner([-1, 0, 0])?, ([-1, 0, 0], [31999, 0, 0]));
    /// assert!(profile.owner([128000, 0, 0]).is_err());
    /// # Ok::<(), rou_core::Error>(())
    /// ```
    pub fn owner(&self, position: Point) -> Result<(Point, Point)> {
        self.validate()?;
        vector(&position)?;
        let mut chunk = [0; 3];
        let mut offset = [0; 3];
        for axis in 0..3 {
            let relative = position[axis] - self.origin_mm[axis];
            chunk[axis] = relative.div_euclid(self.chunk_size_mm[axis]);
            offset[axis] = relative.rem_euclid(self.chunk_size_mm[axis]);
        }
        require(
            self.contains_chunk(chunk),
            "outside_world",
            "position outside realm",
        )?;
        Ok((chunk, offset))
    }
    /// Return all chunks intersecting a world AABB, sorted lexicographically by X/Y/Z.
    ///
    /// Use the exclusive upper bound, so touching the next chunk boundary does not occupy that chunk. Do not clip geometry to the world.
    ///
    /// # Errors
    ///
    /// Return errors for invalid bounds/profiles, endpoints outside the world, or coverage
    /// exceeding [`limits::ENTITY_CHUNKS`] chunks.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use rou_core::spatial::{Bounds, Profile};
    /// let bounds = Bounds { min: [0, 0, 0], max: [32000, 1000, 1000] };
    /// assert_eq!(Profile::default().intersections(&bounds)?, vec![[0, 0, 0]]);
    /// # Ok::<(), rou_core::Error>(())
    /// ```
    pub fn intersections(&self, bounds: &Bounds) -> Result<Vec<Point>> {
        bounds.validate()?;
        let (first, _) = self.owner(bounds.min)?;
        let (last, _) = self.owner(bounds.max.map(|v| v - 1))?;
        range_points(first, last, limits::ENTITY_CHUNKS)
    }
    /// Expand single-chunk `x,y,z` or closed-range `x,y,z..x,y,z` selectors.
    ///
    /// Endpoints may be reversed. Return the sorted, deduplicated union of all selectors;
    /// an empty list produces an empty selection. Selector endpoints are included,
    /// while the world's `max_chunk` remains an exclusive upper bound.
    ///
    /// # Errors
    ///
    /// Propagate profile and [`parse_chunk`] errors. Return `outside_world` for endpoints
    /// outside the world, or `spatial_budget` for excessive coverage.
    pub fn range(&self, selectors: &[String]) -> Result<Vec<Point>> {
        self.validate()?;
        let mut chunks = BTreeSet::new();
        for selector in selectors {
            let (left, right) = selector.split_once("..").unwrap_or((selector, selector));
            let left = parse_chunk(left)?;
            let right = parse_chunk(right)?;
            require(
                self.contains_chunk(left) && self.contains_chunk(right),
                "outside_world",
                "chunk range outside realm",
            )?;
            let first = std::array::from_fn(|i| left[i].min(right[i]));
            let last = std::array::from_fn(|i| left[i].max(right[i]));
            chunks.extend(range_points(first, last, limits::WORLD_CHUNKS)?);
            require(
                chunks.len() <= limits::WORLD_CHUNKS,
                "spatial_budget",
                "selection exceeds chunk budget",
            )?;
        }
        Ok(chunks.into_iter().collect())
    }
}

document!(
    /// An axis-aligned bounding box `[min, max)` in integer millimetres.
    ///
    /// Deserialization checks only coordinate budgets; [`Self::validate`] also requires
    /// `min < max` on every axis. Context determines local versus world coordinates;
    /// use [`Self::translated`] to move from local to world space.
    Bounds {
        /// Inclusive minimum coordinates.
        #[serde(deserialize_with = "json::vector")]
        min: Point,
        /// Exclusive maximum coordinates.
        #[serde(deserialize_with = "json::vector")]
        max: Point,
    }
);
impl Bounds {
    /// Check both endpoints' integer coordinate budgets, allowing zero thickness or reversed endpoints.
    ///
    /// # Errors
    ///
    /// Return `invalid_coordinate` if either endpoint exceeds the precision budget.
    pub fn validate_shape(&self) -> Result<()> {
        vector(&self.min)?;
        vector(&self.max)
    }
    /// Check coordinate budgets and strictly positive thickness on every axis.
    ///
    /// # Errors
    ///
    /// Propagate coordinate errors; return `invalid_bounds` if `min >= max` on any axis.
    pub fn validate(&self) -> Result<()> {
        self.validate_shape()?;
        require(
            (0..3).all(|i| self.min[i] < self.max[i]),
            "invalid_bounds",
            "empty or inverted AABB",
        )
    }
    /// Translate a local AABB by a millimetre offset into another coordinate system with the same axes.
    ///
    /// Does not rotate, scale or check a particular profile's world bounds.
    ///
    /// # Errors
    ///
    /// Propagate coordinate/bounds errors for invalid input bounds, offsets or translated endpoints.
    pub fn translated(&self, position: Point) -> Result<Self> {
        self.validate()?;
        vector(&position)?;
        let result = Self {
            min: std::array::from_fn(|i| self.min[i] + position[i]),
            max: std::array::from_fn(|i| self.max[i] + position[i]),
        };
        result.validate()?;
        Ok(result)
    }
}

fn range_points(first: Point, last: Point, limit: usize) -> Result<Vec<Point>> {
    let count: i128 = (0..3).map(|i| i128::from(last[i] - first[i] + 1)).product();
    require(
        count > 0 && count <= limit as i128,
        "spatial_budget",
        "chunk coverage budget exceeded",
    )?;
    let mut result = Vec::with_capacity(count as usize);
    for x in first[0]..=last[0] {
        for y in first[1]..=last[1] {
            for z in first[2]..=last[2] {
                result.push([x, y, z]);
            }
        }
    }
    Ok(result)
}

/// Format a chunk index as canonical decimal `x,y,z` without spaces.
///
/// Does not validate the input range; external inputs should first pass [`parse_chunk`] or [`vector`].
pub fn chunk_key(chunk: Point) -> String {
    format!("{},{},{}", chunk[0], chunk[1], chunk[2])
}
/// Parse a canonically spelled `x,y,z` chunk index without checking world membership.
///
/// Reject spaces, plus signs, leading zeroes, `-0` and floating-point spellings.
///
/// # Errors
///
/// Return `invalid_selector` for an invalid component count, integer syntax or spelling,
/// or `invalid_coordinate` for an over-budget component.
pub fn parse_chunk(value: &str) -> Result<Point> {
    let parts: Vec<_> = value.split(',').collect();
    require(
        parts.len() == 3,
        "invalid_selector",
        "chunk must have three coordinates",
    )?;
    let mut result = [0; 3];
    for (component, part) in result.iter_mut().zip(parts) {
        *component = part
            .parse()
            .map_err(|_| Error::new("invalid_selector", "invalid chunk coordinate"))?;
        require(
            component.to_string() == part,
            "invalid_selector",
            "chunk coordinate must use canonical decimal spelling",
        )?;
    }
    vector(&result)?;
    Ok(result)
}

/// Build a `chunk_key -> entity IDs` index from each entity's world AABB.
///
/// Include occupied chunks only and sort their IDs. Do not deduplicate repeated entity
/// IDs or validate other entity fields. Callers must ensure unique IDs and validate
/// the full catalog. An empty entity list does not trigger profile validation.
///
/// # Errors
///
/// Propagate bounds, world-range or coverage-budget errors from [`Profile::intersections`].
pub fn build_index(
    profile: &Profile,
    entities: &[PreviewEntity],
) -> Result<BTreeMap<String, Vec<String>>> {
    let mut index: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for entity in entities {
        for chunk in profile.intersections(&entity.bounds_mm)? {
            index
                .entry(chunk_key(chunk))
                .or_default()
                .push(entity.id.clone());
        }
    }
    for ids in index.values_mut() {
        ids.sort();
    }
    Ok(index)
}
