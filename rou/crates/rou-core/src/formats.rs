//! The single source of constants for document formats, spatial profiles and native ABI identifiers.
//!
//! Readers accept only these known identifiers; a matching identifier does not imply compatibility with the retired 2.0 layout.
/// Format identifier for the immutable Realm root document.
pub const REALM: &str = "rou-realm/0.1";
/// Format identifier for a model definition.
pub const MODEL: &str = "rou-model/0.1";
/// Format identifier for an entity placement.
pub const ENTITY: &str = "rou-entity/0.1";
/// Format identifier for a model build record.
pub const BUILD: &str = "rou-build/0.2";
/// Blender export response format, also included in the build input digest.
pub const BLENDER_EXPORT: &str = "rou-blender-export/0.2";
/// Format identifier for an in-memory projection; never written as a versioned file.
pub const VIEW: &str = "rou-view/0.1";
/// Format identifier for the viewer catalog.
pub const PREVIEW: &str = "rou-preview/0.2";
/// Format identifier for a browse-bundle sidecar bound to a fixed Git revision.
pub const GIT_VIEW: &str = "rou-git-view/0.1";
/// Identifier for the current finite Cartesian spatial profile.
pub const PROFILE: &str = "bounded-cartesian-v1-experiment";
/// Interface version used by the C ABI and GDExtension startup self-checks.
pub const ABI_VERSION: u32 = 1;
