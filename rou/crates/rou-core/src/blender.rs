//! Pure coordinate, bounds and relative-path conversion between Blender Z-up and Realm/Godot Y-up.
//!
//! The coordinate mapping is `(x, y, z) -> (x, z, -y)`; hosts handle bpy, file existence and symlinks.
//!
//! ```rust
//! use rou_core::blender;
//!
//! let position_mm = blender::placement([1.0, 2.0, 3.0])?;
//! assert_eq!(position_mm, [1000, 3000, -2000]);
//! assert_eq!(blender::inverse(position_mm)?, [1.0, 2.0, 3.0]);
//! assert_eq!(blender::relative_path("sources/house/main.blend", "//../wood.png")?,
//!            "sources/wood.png");
//! # Ok::<(), rou_core::Error>(())
//! ```
use crate::{
    Result,
    error::require,
    limits, paths,
    spatial::{Bounds, Point},
};

fn millimetres(value: f64) -> Result<i64> {
    let value = (value * 1000.0).round();
    require(
        value.is_finite() && value.abs() <= limits::COORDINATE_MM as f64,
        "invalid_coordinate",
        "Blender coordinate exceeds precision bound",
    )?;
    Ok(value as i64)
}

/// Convert a Blender Z-up position in metres to Y-up integer millimetres.
///
/// Multiply each axis by 1000 and round to the nearest integer, with half-millimetre ties away from zero.
///
/// # Errors
///
/// Return `invalid_coordinate` if a rounded coordinate is nonfinite or exceeds the
/// millimetre precision budget. Does not check a particular Realm's world bounds.
pub fn placement(position: [f64; 3]) -> Result<Point> {
    Ok([
        millimetres(position[0])?,
        millimetres(position[2])?,
        millimetres(-position[1])?,
    ])
}

/// Convert a Y-up integer millimetre position back to Blender Z-up metres `(x, -z, y)`.
///
/// # Errors
///
/// Return `invalid_coordinate` if the millimetre vector exceeds the precision budget.
pub fn inverse(position: Point) -> Result<[f64; 3]> {
    crate::spatial::vector(&position)?;
    Ok([
        position[0] as f64 / 1000.0,
        -position[2] as f64 / 1000.0,
        position[1] as f64 / 1000.0,
    ])
}

/// Compute a conservative integer millimetre AABB from points already in Y-up metres.
///
/// Floor each minimum and ceil each maximum. If they are equal, extend the maximum
/// by 1 mm so that planes and lines have positive thickness. Does not transform axes again.
///
/// # Errors
///
/// Return `invalid_bounds` for an empty point set, or `invalid_coordinate` for nonfinite
/// or over-budget coordinates. The expanded box must also pass [`Bounds::validate`].
pub fn bounds(points: &[[f64; 3]]) -> Result<Bounds> {
    require(
        !points.is_empty(),
        "invalid_bounds",
        "no geometry bounds supplied",
    )?;
    let mut lower = [f64::INFINITY; 3];
    let mut upper = [f64::NEG_INFINITY; 3];
    for point in points {
        for axis in 0..3 {
            require(
                point[axis].is_finite()
                    && (point[axis] * 1000.0).abs() <= limits::COORDINATE_MM as f64,
                "invalid_coordinate",
                "invalid geometry coordinate",
            )?;
            lower[axis] = lower[axis].min(point[axis] * 1000.0);
            upper[axis] = upper[axis].max(point[axis] * 1000.0);
        }
    }
    let min = lower.map(|v| v.floor() as i64);
    let mut max = upper.map(|v| v.ceil() as i64);
    for axis in 0..3 {
        if min[axis] == max[axis] {
            max[axis] += 1;
        }
    }
    let bounds = Bounds { min, max };
    bounds.validate()?;
    Ok(bounds)
}

/// Convert a Blender Z-up metre AABB to a conservative Y-up millimetre AABB.
///
/// # Errors
/// Return `invalid_bounds` for reversed axes, or propagate coordinate budget errors.
pub fn bounds_native([lo, hi]: [[f64; 3]; 2]) -> Result<Bounds> {
    require(
        (0..3).all(|a| lo[a] <= hi[a]),
        "invalid_bounds",
        "reversed Blender bounds",
    )?;
    bounds(&[[lo[0], lo[2], -hi[1]], [hi[0], hi[2], -lo[1]]])
}

/// Resolve an asset reference relative to its owning `.blend` directory into a repository-relative path.
///
/// Treat `//` as Blender's relative-path prefix; references without it use the same base.
/// Allow `.` and `..` provided they stay within the Realm root. Does not access disk or normalize Unicode.
///
/// # Errors
///
/// Return `unsafe_path` for an unsafe source path, absolute reference, root escape or
/// unsafe reduced path. Hosts must check missing files and symlinks separately.
pub fn relative_path(source: &str, reference: &str) -> Result<String> {
    paths::safe(source)?;
    let reference = reference.strip_prefix("//").unwrap_or(reference);
    require(
        !reference.is_empty()
            && !reference.starts_with('/')
            && !reference.starts_with('~')
            && !reference.contains('\\')
            && !reference.contains(':'),
        "unsafe_path",
        "use a Blender relative path",
    )?;
    let mut parts: Vec<_> = source.split('/').collect();
    parts.pop();
    for part in reference.split('/') {
        match part {
            "." => {}
            ".." => {
                require(
                    parts.pop().is_some(),
                    "unsafe_path",
                    "dependency escapes realm",
                )?;
            }
            _ => parts.push(part),
        }
    }
    let result = parts.join("/");
    paths::safe(&result)?;
    Ok(result)
}
