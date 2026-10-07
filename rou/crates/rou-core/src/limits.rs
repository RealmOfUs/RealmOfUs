//! Parsing safety ceilings, viewer defaults, LOD export policy and advisory build references.
//!
//! Unless stated otherwise, each limit is an inclusive maximum. Hosts should use the
//! same byte budgets before reading files. Constants alone do not limit allocation or
//! perform IO; the corresponding parsing and validation functions enforce them.
/// Safety: maximum bytes for an ordinary JSON input (4 MiB).
pub const JSON_BYTES: usize = 4 * 1024 * 1024;
/// Safety: maximum strict-JSON nesting depth, counting the root value at depth 0.
pub const JSON_DEPTH: usize = 40;
/// Safety: maximum bytes for one GLB container (256 MiB).
pub const GLB_BYTES: usize = 256 * 1024 * 1024;
/// Safety: maximum bytes of the GLB JSON chunk; geometry lives in the binary chunk.
/// Retains the pre-M3 parser ceiling while allowing larger binary payloads.
pub const GLB_JSON_BYTES: usize = 8 * 1024 * 1024;
/// Safety: maximum bytes in a source/dependency payload descriptor (1 GiB).
pub const FILE_BYTES: u64 = 1024 * 1024 * 1024;
/// Safety: maximum text bytes recognized as a Git LFS pointer.
pub const POINTER_BYTES: usize = 1024;
/// Safety: maximum sum of primitive POSITION accessor counts in one GLB.
pub const VERTICES: u64 = 16_777_216;
/// Safety: maximum total indices in one GLB and maximum count of a single accessor.
pub const INDICES: u64 = 50_331_648;
/// Safety: maximum number of nodes in one GLB.
pub const NODES: usize = 65_536;
/// Safety: maximum GLB node hierarchy depth in edges, starting at depth 0.
pub const NODE_DEPTH: usize = 256;
/// Safety: maximum GLB top-level array length and primitive count per mesh.
pub const GLB_ARRAY_ITEMS: usize = 1_048_576;
/// Safety: maximum pixels along each axis of an embedded PNG/JPEG.
pub const IMAGE_DIMENSION: u32 = 16_384;
/// Safety: decoded-image budget for one GLB, estimated at four bytes per pixel.
pub const IMAGE_DECODED_BYTES: u64 = 4 * 1024 * 1024 * 1024;
/// Safety: maximum absolute millimetre component, also used to check integer spatial vectors.
pub const COORDINATE_MM: i64 = 1_000_000_000;
/// Safety: maximum absolute camera coordinate in metres; allows a full world-diagonal height outside the world.
pub const VIEW_COORDINATE_METRES: f64 = 4.0 * COORDINATE_MM as f64 / 1000.0;
/// Safety: maximum middle-radius times hysteresis, in metres, independent of the default multiplier.
pub const VIEW_RETENTION_METRES: f64 = 4_600_000.0;
/// Safety: maximum chunk edge length per axis, in millimetres; the minimum is 1 mm.
pub const CHUNK_SIZE_MM: i64 = 64_000;
/// Safety: maximum chunk count in the finite world or a selection.
pub const WORLD_CHUNKS: usize = 32_768;
/// Safety: maximum chunks overlapped by one entity AABB.
pub const ENTITY_CHUNKS: usize = 2048;
/// Safety: separate limits on model count and entity count, not a limit on their sum.
pub const WORLD_RECORDS: usize = 4096;
/// Safety: maximum dependency paths per model, excluding its source file.
pub const DEPENDENCIES: usize = 4096;
/// Safety: maximum source dependency graph depth in edges, starting at depth 0.
pub const DEPENDENCY_DEPTH: usize = 64;
/// Safety: maximum ASCII byte length of a stable identifier.
pub const ID_BYTES: usize = 80;
/// Safety: maximum Unicode scalar values in a label or Blender export object name.
pub const LABEL_CHARS: usize = 256;

/// Viewer default: near-detail distance to a chunk AABB, in metres.
pub const VIEW_NEAR_METRES: f64 = 48.0;
/// Viewer default: mesh residency distance to a chunk AABB, in metres.
pub const VIEW_MID_METRES: f64 = 200.0;
/// Viewer default: metadata-impostor visibility distance to a chunk AABB, in metres.
pub const VIEW_FAR_METRES: f64 = 4000.0;
/// Viewer default: residency multiplier for already loaded assets at the middle boundary.
pub const VIEW_HYSTERESIS: f64 = 1.15;

// Changes to published LOD policy require an exporter format bump.
/// LOD export policy: target triangle ratio for the middle-detail asset.
pub const LOD1_RATIO: f64 = 0.25;
/// LOD export policy: minimum triangle count before simplifying a unique mesh.
pub const LOD1_MIN_TRIANGLES: usize = 500;
/// LOD export policy: maximum image edge after aspect-preserving resampling.
pub const LOD1_IMAGE_DIMENSION: u32 = 1024;

/// Reference: warn when a GLB exceeds 32 MiB; never rejects input.
pub const GLB_WARN_BYTES: usize = 32 * 1024 * 1024;
/// Reference: warn when primitive POSITION counts exceed 500,000; never rejects input.
pub const VERTICES_WARN: u64 = 500_000;
/// Reference: warn when an image edge exceeds 4096 pixels; never rejects input.
pub const IMAGE_WARN_DIMENSION: u32 = 4096;
/// Reference: warn when the decoded-image estimate exceeds 512 MiB; never rejects input.
pub const IMAGE_DECODED_WARN_BYTES: u64 = 512 * 1024 * 1024;
