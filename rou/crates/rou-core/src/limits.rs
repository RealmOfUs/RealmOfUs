//! Shared numeric ranges and resource budgets.
//!
//! Unless stated otherwise, each limit is an inclusive maximum. Hosts should use the
//! same byte budgets before reading files. Constants alone do not limit allocation or
//! perform IO; the corresponding parsing and validation functions enforce them.
/// Maximum bytes for an ordinary JSON input (4 MiB).
pub const JSON_BYTES: usize = 4 * 1024 * 1024;
/// Maximum strict-JSON nesting depth, counting the root value at depth 0.
pub const JSON_DEPTH: usize = 40;
/// Maximum bytes for one GLB (8 MiB), also applied to its JSON chunk.
pub const GLB_BYTES: usize = 8 * 1024 * 1024;
/// Maximum bytes in a source/dependency payload descriptor (1 GiB).
pub const FILE_BYTES: u64 = 1024 * 1024 * 1024;
/// Maximum text bytes recognized as a Git LFS pointer.
pub const POINTER_BYTES: usize = 1024;
/// Maximum sum of primitive POSITION accessor counts in one GLB.
pub const VERTICES: u64 = 100_000;
/// Maximum total indices in one GLB and maximum count of a single accessor.
pub const INDICES: u64 = 300_000;
/// Maximum number of nodes in one GLB.
pub const NODES: usize = 512;
/// Maximum GLB node hierarchy depth in edges, starting at depth 0.
pub const NODE_DEPTH: usize = 32;
/// Maximum GLB top-level array length and primitive count per mesh.
pub const GLB_ARRAY_ITEMS: usize = 8192;
/// Maximum pixels along each axis of an embedded PNG/JPEG.
pub const IMAGE_DIMENSION: u32 = 2048;
/// Decoded-image budget for one GLB, estimated at four bytes per pixel.
pub const IMAGE_DECODED_BYTES: u64 = 64 * 1024 * 1024;
/// Maximum absolute millimetre component, also used to check integer spatial vectors.
pub const COORDINATE_MM: i64 = 1_000_000_000;
/// Maximum absolute camera coordinate in metres; allows a full world-diagonal height outside the world.
pub const VIEW_COORDINATE_METRES: f64 = 4.0 * COORDINATE_MM as f64 / 1000.0;
/// Default near-detail distance to a chunk AABB, in metres.
pub const VIEW_NEAR_METRES: f64 = 48.0;
/// Default full-asset residency distance to a chunk AABB, in metres.
pub const VIEW_MID_METRES: f64 = 200.0;
/// Default metadata-impostor visibility distance to a chunk AABB, in metres.
pub const VIEW_FAR_METRES: f64 = 4000.0;
/// Default residency multiplier for already loaded assets at the middle boundary.
pub const VIEW_HYSTERESIS: f64 = 1.15;
/// Maximum middle-radius times hysteresis, in metres, independent of the default multiplier.
pub const VIEW_RETENTION_METRES: f64 = 4_600_000.0;
/// Maximum chunk edge length per axis, in millimetres; the minimum is 1 mm.
pub const CHUNK_SIZE_MM: i64 = 64_000;
/// Maximum chunk count in the finite world or a selection.
pub const WORLD_CHUNKS: usize = 32_768;
/// Maximum chunks overlapped by one entity AABB.
pub const ENTITY_CHUNKS: usize = 2048;
/// Separate limits on model count and entity count, not a limit on their sum.
pub const WORLD_RECORDS: usize = 4096;
/// Maximum dependency paths per model, excluding its source file.
pub const DEPENDENCIES: usize = 4096;
/// Maximum source dependency graph depth in edges, starting at depth 0.
pub const DEPENDENCY_DEPTH: usize = 64;
/// Maximum ASCII byte length of a stable identifier.
pub const ID_BYTES: usize = 80;
/// Maximum Unicode scalar values in a label or Blender export object name.
pub const LABEL_CHARS: usize = 256;
