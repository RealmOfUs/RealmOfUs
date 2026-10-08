//! A resource-bounded precheck for static GLB 2.0 assets.
//!
//! Check embedded data layouts, static triangles, node graphs and PNG/JPEG size budgets.
//! Does not load external URIs or decode images, and is not a replacement for a complete
//! glTF validator or a sandbox around the native importer.
use crate::{
    Error, Result,
    documents::{Descriptor, Texture},
    error::require,
    image, json, limits,
};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::{BTreeMap, BTreeSet};

fn check(condition: bool, message: &str) -> Result<()> {
    require(condition, "invalid_glb", message)
}
fn object(value: &Value) -> Result<&Map<String, Value>> {
    value
        .as_object()
        .ok_or_else(|| Error::new("invalid_glb", "expected glTF object"))
}
fn array<'a>(value: &'a Value, key: &str) -> Result<&'a [Value]> {
    match value.get(key) {
        None => Ok(&[]),
        Some(Value::Array(values)) => Ok(values),
        _ => Err(Error::new("invalid_glb", format!("{key} must be an array"))),
    }
}
fn number(value: &Value) -> Result<u64> {
    json::integer(value, 0, limits::FILE_BYTES as i64).map(|v| v as u64)
}
fn field(value: &Value, key: &str) -> Result<u64> {
    number(
        value
            .get(key)
            .ok_or_else(|| Error::new("invalid_glb", format!("missing {key}")))?,
    )
}
fn optional(value: &Value, key: &str, fallback: u64) -> Result<u64> {
    value.get(key).map_or(Ok(fallback), number)
}
fn index(value: &Value, count: usize) -> Result<usize> {
    let index = number(value)? as usize;
    check(index < count, "invalid glTF index")?;
    Ok(index)
}
fn word(data: &[u8], offset: usize) -> Result<u32> {
    let bytes = data
        .get(offset..offset + 4)
        .ok_or_else(|| Error::new("invalid_glb", "truncated GLB header"))?;
    Ok(u32::from_le_bytes(bytes.try_into().expect("four bytes")))
}

fn supported(value: &Value) -> Result<()> {
    match value {
        Value::Object(map) => {
            check(!map.contains_key("uri"), "URI outside images forbidden")?;
            check(!map.contains_key("extensions"), "extensions unsupported")?;
            for child in map.values() {
                supported(child)?;
            }
        }
        Value::Array(values) => {
            for child in values {
                supported(child)?;
            }
        }
        _ => {}
    }
    Ok(())
}

/// Resource totals for one prechecked GLB, not the rendering cost of instantiated scenes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Report {
    /// Total GLB container bytes, including JSON/BIN chunks and padding.
    pub bytes: usize,
    /// Sum of all primitive POSITION accessor counts, without multiplying by node instances.
    pub vertices: u64,
    /// Sum of primitive index accessor counts; non-indexed primitives do not contribute.
    pub indices: u64,
    /// Length of the glTF nodes array.
    pub nodes: usize,
    /// Sum of unique referenced image width * height; the budget assumes four bytes per pixel.
    pub image_pixels: u64,
    /// Largest referenced image edge, or zero when there are no images.
    pub max_image_dimension: u32,
    /// Estimated decoded image bytes, using four bytes per pixel.
    pub image_decoded_bytes: u64,
    /// Length of the glTF meshes array.
    pub meshes: usize,
    /// Sorted unique external image filenames; image bytes are checked separately.
    pub images: Vec<String>,
}

impl Report {
    /// Require an exact texture declaration for this GLB, with no missing or extra URI.
    pub fn match_textures(&self, textures: &[Texture]) -> Result<()> {
        let names = textures
            .iter()
            .map(|t| t.filename())
            .collect::<BTreeSet<_>>();
        check(
            names.len() == textures.len()
                && names
                    .iter()
                    .copied()
                    .eq(self.images.iter().map(String::as_str)),
            "GLB image references differ from declared textures",
        )
    }
    /// Combine geometry statistics with exactly the referenced, independently checked images.
    pub fn with_images(mut self, images: &BTreeMap<String, image::Report>) -> Result<Self> {
        check(
            self.images.iter().eq(images.keys()),
            "GLB image statistics set mismatch",
        )?;
        self.image_pixels = 0;
        self.max_image_dimension = 0;
        for (name, stats) in images {
            stats.validate()?;
            check(
                name.ends_with(&format!(".{}", stats.extension)),
                "image signature and URI extension differ",
            )?;
            self.image_pixels += stats.pixels;
            self.max_image_dimension = self.max_image_dimension.max(stats.width).max(stats.height);
            check(
                self.image_pixels * 4 <= limits::IMAGE_DECODED_BYTES,
                "GLB safety limit exceeded: IMAGE_DECODED_BYTES (4 GiB)",
            )?;
        }
        self.image_decoded_bytes = self.image_pixels * 4;
        Ok(self)
    }
    /// Return advisory build warnings; these references never affect validation.
    pub fn warnings(&self) -> Vec<String> {
        let mut warnings = Vec::new();
        if self.bytes > limits::GLB_WARN_BYTES {
            warnings.push(format!(
                "GLB bytes {} exceed reference {}",
                self.bytes,
                limits::GLB_WARN_BYTES
            ));
        }
        if self.vertices > limits::VERTICES_WARN {
            warnings.push(format!(
                "vertices {} exceed reference {}",
                self.vertices,
                limits::VERTICES_WARN
            ));
        }
        if self.max_image_dimension > limits::IMAGE_WARN_DIMENSION {
            warnings.push(format!(
                "image edge {} exceeds reference {}",
                self.max_image_dimension,
                limits::IMAGE_WARN_DIMENSION
            ));
        }
        if self.image_decoded_bytes > limits::IMAGE_DECODED_WARN_BYTES {
            warnings.push(format!(
                "decoded image bytes {} exceed reference {}",
                self.image_decoded_bytes,
                limits::IMAGE_DECODED_WARN_BYTES
            ));
        }
        warnings
    }
}

/// Check complete GLB bytes and return budget statistics without modifying the input.
///
/// Support one embedded geometry buffer, static triangles and canonical external PNG/JPEG image URIs.
/// Reject non-image URIs, extensions, animations, skins, cameras, morph targets and sparse accessors.
/// Check chunk lengths, view/accessor bounds, node indices/cycles/depth and geometry/image
/// budgets, but not every glTF semantic constraint or the vertex contents of buffers.
///
/// # Errors
///
/// Unsupported or malformed GLBs normally return `invalid_glb`. Embedded JSON and
/// integer parsing retain core codes such as `invalid_json`, `json_budget` or `invalid_integer`.
pub fn precheck(data: &[u8]) -> Result<Report> {
    inspect(data, false)
}

fn inspect(data: &[u8], embedded: bool) -> Result<Report> {
    check(data.len() >= 20, "GLB size below minimum (20 bytes)")?;
    check(
        data.len() <= limits::GLB_BYTES,
        "GLB safety limit exceeded: GLB_BYTES (256 MiB)",
    )?;
    check(
        word(data, 0)? == 0x46546c67
            && word(data, 4)? == 2
            && word(data, 8)? as usize == data.len(),
        "invalid GLB header",
    )?;
    let length = word(data, 12)? as usize;
    check(
        word(data, 16)? == 0x4e4f534a && length % 4 == 0 && length <= data.len() - 20,
        "invalid GLB JSON chunk",
    )?;
    check(
        length <= limits::GLB_JSON_BYTES,
        "GLB safety limit exceeded: GLB_JSON_BYTES (8 MiB)",
    )?;
    let document = json::strict_with_limit(&data[20..20 + length], limits::GLB_JSON_BYTES)?;
    let end = 20 + length;
    let binary = if end == data.len() {
        &[][..]
    } else {
        let length = word(data, end)? as usize;
        check(
            word(data, end + 4)? == 0x004e4942 && length % 4 == 0 && end + 8 + length == data.len(),
            "invalid GLB BIN chunk",
        )?;
        &data[end + 8..]
    };
    let root = object(&document)?;
    check(
        document
            .get("asset")
            .and_then(|v| v.get("version"))
            .and_then(Value::as_str)
            == Some("2.0"),
        "unsupported glTF version",
    )?;
    for (key, value) in root {
        check(
            key != "uri" && key != "extensions",
            "root URI/extensions unsupported",
        )?;
        if key != "images" {
            supported(value)?;
        }
    }
    for value in root.values() {
        if let Some(values) = value.as_array() {
            check(
                values.len() <= limits::GLB_ARRAY_ITEMS,
                "GLB safety limit exceeded: GLB_ARRAY_ITEMS",
            )?;
        }
    }
    for key in [
        "extensionsUsed",
        "extensionsRequired",
        "animations",
        "skins",
        "cameras",
    ] {
        check(
            array(&document, key)?.is_empty(),
            "only static meshes without extensions are supported",
        )?;
    }
    let buffers = array(&document, "buffers")?;
    check(buffers.len() == 1, "exactly one embedded buffer required")?;
    let buffer_size = field(&buffers[0], "byteLength")?;
    check(
        buffer_size <= binary.len() as u64 && binary.len() as u64 - buffer_size <= 3,
        "buffer length mismatch",
    )?;
    let views = array(&document, "bufferViews")?;
    for view in views {
        object(view)?;
        check(optional(view, "buffer", 0)? == 0, "invalid buffer index")?;
        check(
            optional(view, "byteOffset", 0)? + field(view, "byteLength")? <= buffer_size,
            "bufferView overrun",
        )?;
    }
    let accessors = array(&document, "accessors")?;
    for accessor in accessors {
        check(
            !object(accessor)?.contains_key("sparse"),
            "sparse accessors unsupported",
        )?;
        let view_index = index(
            accessor.get("bufferView").unwrap_or(&Value::Null),
            views.len(),
        )?;
        let width = match field(accessor, "componentType")? {
            5120 | 5121 => 1,
            5122 | 5123 => 2,
            5125 | 5126 => 4,
            _ => {
                return Err(Error::new(
                    "invalid_glb",
                    "unsupported accessor component type",
                ));
            }
        };
        let components = match accessor.get("type").and_then(Value::as_str) {
            Some("SCALAR") => 1,
            Some("VEC2") => 2,
            Some("VEC3") => 3,
            Some("VEC4") => 4,
            Some("MAT4") => 16,
            _ => return Err(Error::new("invalid_glb", "unsupported accessor type")),
        };
        let count = field(accessor, "count")?;
        check(
            count <= limits::INDICES,
            "GLB safety limit exceeded: INDICES (accessor count)",
        )?;
        let element = width * components;
        let view = &views[view_index];
        let stride = optional(view, "byteStride", element)?;
        let offset = optional(accessor, "byteOffset", 0)?;
        check(
            stride >= element && offset % width == 0 && stride % width == 0,
            "invalid accessor stride/alignment",
        )?;
        let used = if count == 0 {
            0
        } else {
            (count - 1) * stride + element
        };
        check(
            offset + used <= field(view, "byteLength")?,
            "accessor overrun",
        )?;
    }
    let meshes = array(&document, "meshes")?;
    let mut vertices = 0;
    let mut indices = 0;
    for mesh in meshes {
        object(mesh)?;
        let primitives = array(mesh, "primitives")?;
        check(!primitives.is_empty(), "GLB mesh has no primitives")?;
        check(
            primitives.len() <= limits::GLB_ARRAY_ITEMS,
            "GLB safety limit exceeded: GLB_ARRAY_ITEMS (primitives)",
        )?;
        for primitive in primitives {
            let primitive_object = object(primitive)?;
            check(
                optional(primitive, "mode", 4)? == 4 && !primitive_object.contains_key("targets"),
                "static triangles required",
            )?;
            let attributes = object(primitive.get("attributes").unwrap_or(&Value::Null))?;
            for value in attributes.values() {
                index(value, accessors.len())?;
            }
            let position = &accessors[index(
                attributes.get("POSITION").unwrap_or(&Value::Null),
                accessors.len(),
            )?];
            check(
                position.get("type").and_then(Value::as_str) == Some("VEC3")
                    && field(position, "componentType")? == 5126,
                "invalid position accessor",
            )?;
            vertices += field(position, "count")?;
            if let Some(value) = primitive.get("indices") {
                let accessor = &accessors[index(value, accessors.len())?];
                check(
                    accessor.get("type").and_then(Value::as_str) == Some("SCALAR")
                        && [5121, 5123, 5125].contains(&field(accessor, "componentType")?),
                    "invalid triangle indices",
                )?;
                indices += field(accessor, "count")?;
            }
            check(
                vertices <= limits::VERTICES && indices <= limits::INDICES,
                "GLB safety limit exceeded: VERTICES or INDICES",
            )?;
        }
    }
    check(vertices > 0, "empty geometry")?;
    let nodes = array(&document, "nodes")?;
    check(!nodes.is_empty(), "GLB has no nodes")?;
    check(
        nodes.len() <= limits::NODES,
        "GLB safety limit exceeded: NODES",
    )?;
    let mut parents = vec![false; nodes.len()];
    for node in nodes {
        let map = object(node)?;
        check(
            !map.contains_key("skin")
                && !map.contains_key("camera")
                && !map.contains_key("weights"),
            "static mesh nodes required",
        )?;
        if let Some(mesh) = node.get("mesh") {
            index(mesh, meshes.len())?;
        }
        for child in array(node, "children")? {
            let child = index(child, nodes.len())?;
            check(!parents[child], "multiple node parents")?;
            parents[child] = true;
        }
    }
    let mut active = vec![false; nodes.len()];
    let mut heights = vec![None; nodes.len()];
    for i in 0..nodes.len() {
        node_height(i, nodes, &mut active, &mut heights, 0)?;
    }
    let scenes = array(&document, "scenes")?;
    for scene in scenes {
        object(scene)?;
        let mut roots = std::collections::BTreeSet::new();
        for root in array(scene, "nodes")? {
            let i = index(root, nodes.len())?;
            check(!parents[i] && roots.insert(i), "invalid scene root")?;
        }
    }
    if let Some(scene) = document.get("scene") {
        index(scene, scenes.len())?;
    }
    let mut image_names = BTreeSet::new();
    let mut image_stats = BTreeMap::new();
    for image in array(&document, "images")? {
        let map = object(image)?;
        for (key, value) in map {
            if key != "uri" {
                supported(value)?;
            }
        }
        check(!map.contains_key("extensions"), "extensions unsupported")?;
        let name = if embedded {
            check(
                !map.contains_key("uri"),
                "external images are not accepted by externalise",
            )?;
            let view = &views[index(image.get("bufferView").unwrap_or(&Value::Null), views.len())?];
            let offset = optional(view, "byteOffset", 0)? as usize;
            let size = field(view, "byteLength")? as usize;
            let bytes = &binary[offset..offset + size];
            let stats = image::precheck(bytes)?;
            let mime = if stats.extension == "png" {
                "image/png"
            } else {
                "image/jpeg"
            };
            check(
                image.get("mimeType").and_then(Value::as_str) == Some(mime),
                "embedded image MIME differs from signature",
            )?;
            let name = format!("{}.{}", json::digest(bytes), stats.extension);
            image_stats.insert(name.clone(), stats);
            name
        } else {
            check(
                !map.contains_key("bufferView"),
                "embedded images forbidden in rou-preview/0.3",
            )?;
            let uri = image.get("uri").and_then(Value::as_str).unwrap_or("");
            Texture::uri(uri)?;
            if let Some(mime) = image.get("mimeType") {
                let expected = if uri.ends_with(".png") {
                    "image/png"
                } else {
                    "image/jpeg"
                };
                check(
                    mime.as_str() == Some(expected),
                    "image MIME differs from URI extension",
                )?;
            }
            uri.to_owned()
        };
        image_names.insert(name);
    }
    let report = Report {
        bytes: data.len(),
        vertices,
        indices,
        nodes: nodes.len(),
        image_pixels: 0,
        max_image_dimension: 0,
        image_decoded_bytes: 0,
        meshes: meshes.len(),
        images: image_names.into_iter().collect(),
    };
    if embedded {
        report.with_images(&image_stats)
    } else {
        Ok(report)
    }
}

/// A bounded copy from the original GLB into a rewritten binary chunk.
#[derive(Debug, Clone, Serialize)]
pub struct Segment {
    /// Absolute byte offset in the original GLB.
    pub offset: usize,
    /// Number of original bytes to preserve.
    pub bytes: usize,
    /// Zero padding after this range, restoring four-byte alignment.
    pub padding: usize,
}

/// An image extracted without re-encoding from the original GLB.
#[derive(Debug, Clone, Serialize)]
pub struct ExtractedImage {
    /// Flat content-addressed filename, relative to the rewritten GLB.
    pub uri: String,
    /// Identity of the original encoded bytes.
    pub descriptor: Descriptor,
    /// Independently checked signature and dimension statistics.
    pub metrics: image::Report,
    /// Absolute image byte offset in the original GLB.
    pub offset: usize,
}

/// A pure relocation plan; hosts only copy the specified bytes and publish files.
/// Avoids transporting large images through JSON or reimplementing GLB rules in hosts.
#[derive(Debug, Clone, Serialize)]
pub struct Externalised {
    /// Complete GLB header, JSON chunk and BIN chunk header.
    pub prefix: Vec<u8>,
    /// Geometry ranges in output order; image-only ranges are omitted.
    pub segments: Vec<Segment>,
    /// Deduplicated images in URI order.
    pub textures: Vec<ExtractedImage>,
    /// Expected rewritten GLB identity.
    pub descriptor: Descriptor,
    /// Geometry statistics including the referenced image totals.
    pub metrics: Report,
}
impl Externalised {
    /// Apply the copy plan to its original bytes, within the GLB byte budget.
    pub fn materialize(&self, input: &[u8]) -> Result<Vec<u8>> {
        let mut result = self.prefix.clone();
        for segment in &self.segments {
            let end = segment
                .offset
                .checked_add(segment.bytes)
                .ok_or_else(|| Error::new("invalid_glb", "copy range overflow"))?;
            let bytes = input
                .get(segment.offset..end)
                .ok_or_else(|| Error::new("invalid_glb", "copy range overrun"))?;
            check(
                segment.padding <= 3
                    && result.len() + bytes.len() + segment.padding <= limits::GLB_BYTES,
                "GLB safety limit exceeded: GLB_BYTES",
            )?;
            result.extend_from_slice(bytes);
            result.resize(result.len() + segment.padding, 0);
        }
        Ok(result)
    }
}

/// Extract embedded PNG/JPEG bytes and rewrite a static GLB with flat external URIs.
/// All bufferView indices and offsets are rebuilt; geometry bytes are copied unchanged.
/// Only this conversion entry point accepts embedded images, never the normal reader.
pub fn externalise(data: &[u8]) -> Result<Externalised> {
    let inspected = inspect(data, true)?;
    let length = word(data, 12)? as usize;
    let base = 28 + length;
    let mut document = json::strict_with_limit(&data[20..20 + length], limits::GLB_JSON_BYTES)?;
    let views = array(&document, "bufferViews")?.to_vec();
    let mut images = BTreeMap::new();
    let mut removed = BTreeSet::new();
    if let Some(values) = document.get_mut("images").and_then(Value::as_array_mut) {
        for image in values {
            let old = index(&image["bufferView"], views.len())?;
            removed.insert(old);
            let offset = base + optional(&views[old], "byteOffset", 0)? as usize;
            let size = field(&views[old], "byteLength")? as usize;
            let raw = &data[offset..offset + size];
            let metrics = image::precheck(raw)?;
            let descriptor = Descriptor::of(raw);
            let uri = format!("{}.{}", descriptor.sha256, metrics.extension);
            images.entry(uri.clone()).or_insert(ExtractedImage {
                uri: uri.clone(),
                descriptor,
                metrics,
                offset,
            });
            let image = image.as_object_mut().expect("prechecked image");
            image.remove("bufferView");
            // Keep the verified MIME so importers do not probe JPEG bytes as PNG first.
            image.insert("uri".into(), Value::String(uri));
        }
    }
    let mut mapping = BTreeMap::new();
    let mut kept = Vec::new();
    let mut segments = Vec::new();
    let mut binary_size = 0;
    for (old, view) in views.iter().enumerate() {
        if removed.contains(&old) {
            continue;
        }
        let size = field(view, "byteLength")? as usize;
        let padding = (4 - size % 4) % 4;
        segments.push(Segment {
            offset: base + optional(view, "byteOffset", 0)? as usize,
            bytes: size,
            padding,
        });
        let mut view = view.clone();
        view["buffer"] = 0.into();
        view["byteOffset"] = binary_size.into();
        binary_size += size + padding;
        mapping.insert(old, kept.len());
        kept.push(view);
    }
    if let Some(accessors) = document.get_mut("accessors").and_then(Value::as_array_mut) {
        for accessor in accessors {
            let old = accessor["bufferView"]
                .as_u64()
                .expect("prechecked accessor") as usize;
            let new = mapping.get(&old).ok_or_else(|| {
                Error::new("invalid_glb", "image bufferView is also used by geometry")
            })?;
            accessor["bufferView"] = (*new).into();
        }
    }
    document["bufferViews"] = kept.into();
    document["buffers"][0]["byteLength"] = binary_size.into();
    let mut encoded = json::canonical(&document)?;
    encoded.resize(encoded.len().next_multiple_of(4), b' ');
    check(
        encoded.len() <= limits::GLB_JSON_BYTES,
        "GLB safety limit exceeded: GLB_JSON_BYTES",
    )?;
    let total = 28 + encoded.len() + binary_size;
    check(
        total <= limits::GLB_BYTES,
        "GLB safety limit exceeded: GLB_BYTES",
    )?;
    let mut prefix = Vec::new();
    for word in [
        0x46546c67_u32,
        2,
        total as u32,
        encoded.len() as u32,
        0x4e4f534a,
    ] {
        prefix.extend_from_slice(&word.to_le_bytes());
    }
    prefix.extend_from_slice(&encoded);
    prefix.extend_from_slice(&(binary_size as u32).to_le_bytes());
    prefix.extend_from_slice(&0x004e4942_u32.to_le_bytes());
    let mut result = Externalised {
        prefix,
        segments,
        textures: images.into_values().collect(),
        descriptor: Descriptor::of(&[]),
        metrics: inspected,
    };
    let rewritten = result.materialize(data)?;
    result.descriptor = Descriptor::of(&rewritten);
    result.metrics = precheck(&rewritten)?.with_images(
        &result
            .textures
            .iter()
            .map(|t| (t.uri.clone(), t.metrics.clone()))
            .collect(),
    )?;
    Ok(result)
}

fn node_height(
    i: usize,
    nodes: &[Value],
    active: &mut [bool],
    heights: &mut [Option<usize>],
    depth: usize,
) -> Result<usize> {
    check(
        depth <= limits::NODE_DEPTH && !active[i],
        "cyclic node hierarchy or safety limit exceeded: NODE_DEPTH",
    )?;
    if let Some(height) = heights[i] {
        check(
            depth + height <= limits::NODE_DEPTH,
            "GLB safety limit exceeded: NODE_DEPTH",
        )?;
        return Ok(height);
    }
    active[i] = true;
    let mut height = 0;
    for child in array(&nodes[i], "children")? {
        height = height.max(
            node_height(
                index(child, nodes.len())?,
                nodes,
                active,
                heights,
                depth + 1,
            )? + 1,
        );
    }
    active[i] = false;
    heights[i] = Some(height);
    Ok(height)
}
