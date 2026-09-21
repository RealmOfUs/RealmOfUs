//! A resource-bounded precheck for static GLB 2.0 assets.
//!
//! Check embedded data layouts, static triangles, node graphs and PNG/JPEG size budgets.
//! Does not load external URIs or decode images, and is not a replacement for a complete
//! glTF validator or a sandbox around the native importer.
use crate::{Error, Result, error::require, json, limits};
use serde::Serialize;
use serde_json::{Map, Value};

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
            check(!map.contains_key("uri"), "external/data URI forbidden")?;
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
#[derive(Debug, Clone, Serialize)]
pub struct Report {
    /// Total GLB container bytes, including JSON/BIN chunks and padding.
    pub bytes: usize,
    /// Sum of all primitive POSITION accessor counts, without multiplying by node instances.
    pub vertices: u64,
    /// Sum of primitive index accessor counts; non-indexed primitives do not contribute.
    pub indices: u64,
    /// Length of the glTF nodes array.
    pub nodes: usize,
    /// Sum of embedded image width * height; the budget assumes four bytes per pixel.
    pub image_pixels: u64,
    /// Length of the glTF meshes array.
    pub meshes: usize,
}

/// Check complete GLB bytes and return budget statistics without modifying the input.
///
/// Support one embedded buffer, static triangles and embedded PNG/JPEG images.
/// Reject URIs, extensions, animations, skins, cameras, morph targets and sparse accessors.
/// Check chunk lengths, view/accessor bounds, node indices/cycles/depth and geometry/image
/// budgets, but not every glTF semantic constraint or the vertex contents of buffers.
///
/// # Errors
///
/// Unsupported or malformed GLBs normally return `invalid_glb`. Embedded JSON and
/// integer parsing retain core codes such as `invalid_json`, `json_budget` or `invalid_integer`.
pub fn precheck(data: &[u8]) -> Result<Report> {
    check(
        (20..=limits::GLB_BYTES).contains(&data.len()),
        "GLB byte budget exceeded",
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
    let document = json::strict_with_limit(&data[20..20 + length], limits::GLB_BYTES)?;
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
    supported(&document)?;
    for value in root.values() {
        if let Some(values) = value.as_array() {
            check(
                values.len() <= limits::GLB_ARRAY_ITEMS,
                "glTF top-level array budget exceeded",
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
        check(count <= limits::INDICES, "accessor count budget exceeded")?;
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
        check(
            !primitives.is_empty() && primitives.len() <= limits::GLB_ARRAY_ITEMS,
            "primitive budget exceeded",
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
                "geometry budget exceeded",
            )?;
        }
    }
    check(vertices > 0, "empty geometry")?;
    let nodes = array(&document, "nodes")?;
    check(
        !nodes.is_empty() && nodes.len() <= limits::NODES,
        "node budget exceeded",
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
    let mut pixels = 0_u64;
    for image in array(&document, "images")? {
        object(image)?;
        let view = &views[index(image.get("bufferView").unwrap_or(&Value::Null), views.len())?];
        let offset = optional(view, "byteOffset", 0)? as usize;
        let size = field(view, "byteLength")? as usize;
        let mime = image.get("mimeType").and_then(Value::as_str).unwrap_or("");
        let (width, height) = image_dimensions(&binary[offset..offset + size], mime)?;
        check(
            width > 0
                && height > 0
                && width <= limits::IMAGE_DIMENSION
                && height <= limits::IMAGE_DIMENSION,
            "image dimensions exceed budget",
        )?;
        pixels += u64::from(width) * u64::from(height);
        check(
            pixels * 4 <= limits::IMAGE_DECODED_BYTES,
            "decoded image budget exceeded",
        )?;
    }
    Ok(Report {
        bytes: data.len(),
        vertices,
        indices,
        nodes: nodes.len(),
        image_pixels: pixels,
        meshes: meshes.len(),
    })
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
        "cyclic or excessive node hierarchy",
    )?;
    if let Some(height) = heights[i] {
        check(
            depth + height <= limits::NODE_DEPTH,
            "node hierarchy depth exceeded",
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

/// Read pixel `(width, height)` from a PNG IHDR or JPEG SOF0/1/2 header.
///
/// Does not decode images, verify PNG CRC/JPEG integrity or check dimension budgets.
/// [`precheck`] requires positive dimensions and limits total decoded pixels.
///
/// # Errors
///
/// Return `invalid_glb` for a MIME type other than `image/png`/`image/jpeg`, a damaged header or no supported JPEG frame.
pub fn image_dimensions(data: &[u8], mime: &str) -> Result<(u32, u32)> {
    if mime == "image/png" {
        check(
            data.len() >= 24
                && data.starts_with(b"\x89PNG\r\n\x1a\n")
                && &data[12..16] == b"IHDR"
                && data[8..12] == 13_u32.to_be_bytes(),
            "invalid PNG header",
        )?;
        return Ok((
            u32::from_be_bytes(data[16..20].try_into().expect("four bytes")),
            u32::from_be_bytes(data[20..24].try_into().expect("four bytes")),
        ));
    }
    check(
        mime == "image/jpeg" && data.starts_with(b"\xff\xd8"),
        "unsupported embedded image",
    )?;
    let mut cursor = 2;
    while cursor + 4 <= data.len() {
        check(data[cursor] == 255, "invalid JPEG marker")?;
        while cursor < data.len() && data[cursor] == 255 {
            cursor += 1;
        }
        check(cursor < data.len(), "truncated JPEG marker")?;
        let marker = data[cursor];
        cursor += 1;
        if marker == 0xd8 || marker == 0xd9 || (0xd0..=0xd7).contains(&marker) {
            continue;
        }
        check(cursor + 2 <= data.len(), "truncated JPEG segment")?;
        let size = u16::from_be_bytes([data[cursor], data[cursor + 1]]) as usize;
        check(
            size >= 2 && cursor + size <= data.len(),
            "JPEG segment overrun",
        )?;
        if (0xc0..=0xc2).contains(&marker) {
            check(size >= 8, "truncated JPEG frame")?;
            return Ok((
                u16::from_be_bytes([data[cursor + 5], data[cursor + 6]]) as u32,
                u16::from_be_bytes([data[cursor + 3], data[cursor + 4]]) as u32,
            ));
        }
        cursor += size;
    }
    Err(Error::new("invalid_glb", "JPEG has no supported frame"))
}
