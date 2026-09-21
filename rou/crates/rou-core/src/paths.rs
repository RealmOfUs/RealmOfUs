//! Repository/browse-bundle path constants and pure syntax checks.
//!
//! Paths use `/` separators. The byte interface preserves non-UTF-8 Git paths without
//! Unicode normalization. Hosts must check file existence and symlinks separately.
use crate::{Result, error::require, limits};

/// Filename of repository root rules or a browse catalog.
pub const REALM: &str = "realm.json";
/// Model JSON directory, including its trailing `/`.
pub const MODELS: &str = "realm/models/";
/// Entity JSON directory, including its trailing `/`.
pub const ENTITIES: &str = "realm/entities/";
/// Model GLB and build-record directory, including its trailing `/`.
pub const RUNTIME: &str = "runtime/";
/// Recommended source directory, including its trailing `/`; source paths need not use this prefix.
pub const SOURCES: &str = "sources/";
/// Content-addressed asset directory in a browse bundle, including its trailing `/`.
pub const OBJECTS: &str = "objects/";
/// Sidecar filename for a fixed-version browse bundle.
pub const VERSION: &str = "version.json";
/// Local working directory excluded from version control.
pub const LOCAL: &str = ".rou";
/// Directories always retained in a sparse worktree; their LFS payloads may still be pointers.
pub const BASE_CONE: &[&str] = &["realm", "runtime", "docs"];

/// Check that a stable ID matches `[a-z0-9_-]{1,80}`.
///
/// # Errors
///
/// Return `invalid_id` for empty, overlong, uppercase or otherwise invalid input; does not check namespace uniqueness.
pub fn identity(value: &str) -> Result<()> {
    require(
        !value.is_empty()
            && value.len() <= limits::ID_BYTES
            && value
                .bytes()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'_' || c == b'-'),
        "invalid_id",
        "stable ID must match [a-z0-9_-]{1,80}",
    )
}

/// Check raw Git path bytes for safe repository-relative syntax.
///
/// Allow non-UTF-8 bytes without case or Unicode normalization. Reject absolute paths,
/// a leading `~`, control characters, backslashes, colons, empty/`.`/`..` components,
/// ASCII whitespace at component boundaries, and case-insensitive `.git`/`.rou`
/// components. Hosts must check symlinks.
///
/// # Errors
///
/// Return `unsafe_path` for any of the syntax violations above.
pub fn safe_bytes(value: &[u8]) -> Result<()> {
    require(
        !value.is_empty()
            && value[0] != b'/'
            && value[0] != b'~'
            && !value
                .iter()
                .any(|c| *c < 32 || *c == 127 || *c == b'\\' || *c == b':'),
        "unsafe_path",
        "expected a safe repository-relative path",
    )?;
    for part in value.split(|c| *c == b'/') {
        require(
            !part.is_empty()
                && part != b"."
                && part != b".."
                && !part.eq_ignore_ascii_case(b".git")
                && !part.eq_ignore_ascii_case(b".rou")
                && !part.first().is_some_and(u8::is_ascii_whitespace)
                && !part.last().is_some_and(u8::is_ascii_whitespace),
            "unsafe_path",
            "unsafe path component",
        )?;
    }
    Ok(())
}

/// The UTF-8 string interface to [`safe_bytes`].
///
/// # Errors
///
/// Return `unsafe_path` for unsafe syntax; does not access the filesystem.
pub fn safe(value: &str) -> Result<()> {
    safe_bytes(value.as_bytes())
}
/// Construct `realm/models/<id>.json`.
///
/// Only concatenate the path; callers must validate the ID or SHA-256 shape first.
pub fn model(id: &str) -> String {
    format!("{MODELS}{id}.json")
}
/// Construct `realm/entities/<id>.json`.
///
/// Only concatenate the path; callers must validate the ID or SHA-256 shape first.
pub fn entity(id: &str) -> String {
    format!("{ENTITIES}{id}.json")
}
/// Construct `runtime/<id>.glb` from a model ID.
///
/// Only concatenate the path; callers must validate the ID or SHA-256 shape first.
pub fn asset(id: &str) -> String {
    format!("{RUNTIME}{id}.glb")
}
/// Construct `runtime/<id>.json` from a model ID.
///
/// Only concatenate the path; callers must validate the ID or SHA-256 shape first.
pub fn build(id: &str) -> String {
    format!("{RUNTIME}{id}.json")
}
/// Construct `objects/<sha256>.glb`.
///
/// Only concatenate the path; callers must validate the ID or SHA-256 shape first.
pub fn object(sha256: &str) -> String {
    format!("{OBJECTS}{sha256}.glb")
}
