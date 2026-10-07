#[path = "../../../tests/common/mod.rs"]
mod common;
use rou_core::{glb, limits};
use serde_json::{Value, json};

#[test]
fn triangle_and_truncation() {
    let (document, binary) = common::triangle();
    let bytes = common::glb(&document, &binary);
    let report = glb::precheck(&bytes).unwrap();
    assert_eq!(report.vertices, 3);
    assert_eq!(report.nodes, 1);
    for end in 0..bytes.len() {
        let error = glb::precheck(&bytes[..end]).unwrap_err();
        if end < 20 {
            assert_eq!(error.message, "GLB size below minimum (20 bytes)");
        }
    }
}

#[test]
fn json_chunk_has_an_independent_inclusive_safety_limit() {
    let (document, binary) = common::triangle();
    let original = common::glb(&document, &binary);
    let json_len = u32::from_le_bytes(original[12..16].try_into().unwrap()) as usize;
    for length in [limits::GLB_JSON_BYTES, limits::GLB_JSON_BYTES + 4] {
        // Pad a small valid object without allocating a large serde Value tree.
        let mut bytes = original[..20 + json_len].to_vec();
        bytes.resize(20 + length, b' ');
        bytes.extend(&original[20 + json_len..]);
        let total = bytes.len();
        bytes[8..12].copy_from_slice(&(total as u32).to_le_bytes());
        bytes[12..16].copy_from_slice(&(length as u32).to_le_bytes());
        assert!(total < limits::GLB_BYTES);
        if length == limits::GLB_JSON_BYTES {
            assert_eq!(glb::precheck(&bytes).unwrap().vertices, 3);
        } else {
            assert!(
                glb::precheck(&bytes)
                    .unwrap_err()
                    .message
                    .contains("GLB_JSON_BYTES")
            );
        }
    }
}

#[test]
fn empty_geometry_metadata_and_images_report_lower_bound_errors() {
    let (document, binary) = common::triangle();
    for (pointer, expected) in [
        ("/nodes", "GLB has no nodes"),
        ("/meshes/0/primitives", "GLB mesh has no primitives"),
    ] {
        let mut changed = document.clone();
        *changed.pointer_mut(pointer).unwrap() = json!([]);
        assert_eq!(
            glb::precheck(&common::glb(&changed, &binary))
                .unwrap_err()
                .message,
            expected
        );
    }
    assert_eq!(
        glb::precheck(&textured_geometry(3, 0, 0))
            .unwrap_err()
            .message,
        "image dimensions must be nonzero"
    );
    let (_, mut records) = common::world();
    let asset = &mut records.get_mut("house").unwrap().asset;
    asset.bytes = 19;
    assert_eq!(
        asset.validate_shape().unwrap_err().message,
        "GLB size below minimum (20 bytes)"
    );
}

#[test]
fn migrated_python_glb_corpus_and_shared_budgets() {
    let (document, binary) = common::triangle();
    let cases = [
        ("/buffers/0/uri", json!("https://example.invalid/model.bin")),
        ("/accessors/0/count", json!(100000000)),
        ("/bufferViews/0/byteLength", json!(400)),
        ("/nodes/0/children", json!([0])),
        ("/extensionsUsed", json!(["KHR_draco_mesh_compression"])),
        ("/buffers", json!("bad")),
        ("/meshes/0/primitives/0/attributes", json!({})),
        ("/nodes/0/children", json!([0.5])),
        ("/nodes/0/mesh", json!(999)),
        (
            "/nodes",
            json!([{"children":[1,2]},{"children":[2]},{"mesh":0}]),
        ),
        ("/accessors/0/sparse", json!({})),
        ("/animations", json!([{}])),
        ("/skins", json!([{}])),
        ("/cameras", json!([{}])),
        ("/nodes/0/extensions", json!({})),
        ("/accessors/0/count", json!(true)),
        (
            "/materials",
            json!(vec![json!({}); limits::GLB_ARRAY_ITEMS + 1]),
        ),
        ("/nodes", json!(vec![json!({"mesh":0}); limits::NODES + 1])),
    ];
    for (pointer, value) in cases {
        let mut changed = document.clone();
        let (parent, key) = pointer.rsplit_once('/').unwrap();
        changed
            .pointer_mut(parent)
            .unwrap()
            .as_object_mut()
            .unwrap()
            .insert(key.into(), value);
        assert!(
            glb::precheck(&common::glb(&changed, &binary)).is_err(),
            "{pointer}"
        );
    }
    let mut changed = document.clone();
    changed["images"] = json!([{"bufferView":1,"mimeType":"image/png"}]);
    changed["bufferViews"]
        .as_array_mut()
        .unwrap()
        .push(json!({"buffer":0,"byteOffset":36,"byteLength":24}));
    changed["buffers"][0]["byteLength"] = json!(60);
    let mut image = b"\x89PNG\r\n\x1a\n".to_vec();
    image.extend(13_u32.to_be_bytes());
    image.extend(b"IHDR");
    image.extend(99999_u32.to_be_bytes());
    image.extend(99999_u32.to_be_bytes());
    assert!(glb::precheck(&common::glb(&changed, &[binary, image].concat())).is_err());
}

#[test]
fn hierarchy_depth_is_checked_even_when_children_were_already_visited() {
    let (mut document, binary) = common::triangle();
    let mut nodes = vec![json!({"mesh":0})];
    for i in 1..=limits::NODE_DEPTH + 1 {
        nodes.push(json!({"children":[i-1]}));
    }
    document["nodes"] = Value::Array(nodes);
    document["scenes"] = json!([{"nodes":[limits::NODE_DEPTH + 1]}]);
    assert!(glb::precheck(&common::glb(&document, &binary)).is_err());
}

#[test]
fn glb_json_rejects_non_utf8_duplicate_keys_and_overflow() {
    let (document, binary) = common::triangle();
    let mut bytes = common::glb(&document, &binary);
    bytes[20] = 255;
    assert!(glb::precheck(&bytes).is_err());
    assert!(glb::image_dimensions(b"\xff\xd8\xff\xc0\x00\x00", "image/jpeg").is_err());
}

fn textured_geometry(vertices: usize, dimension: u32, padding: usize) -> Vec<u8> {
    let (mut document, _) = common::triangle();
    let mut binary = vec![0; vertices * 12];
    let offset = binary.len();
    binary.extend(b"\x89PNG\r\n\x1a\n");
    binary.extend(13_u32.to_be_bytes());
    binary.extend(b"IHDR");
    binary.extend(dimension.to_be_bytes());
    binary.extend(dimension.to_be_bytes());
    binary.resize(binary.len().max(padding), 0);
    document["buffers"][0]["byteLength"] = json!(binary.len());
    document["bufferViews"][0]["byteLength"] = json!(vertices * 12);
    document["bufferViews"]
        .as_array_mut()
        .unwrap()
        .push(json!({"buffer":0,"byteOffset":offset,"byteLength":24}));
    document["accessors"][0]["count"] = json!(vertices);
    // Image header fixtures exercise the precheck only, which does not decode PNGs.
    document["images"] = json!([{"bufferView":1,"mimeType":"image/png"}]);
    common::glb(&document, &binary)
}

#[test]
fn former_quality_caps_are_accepted_and_references_only_warn() {
    let bytes = textured_geometry(300_000, 4096, 20 * 1024 * 1024);
    let report = glb::precheck(&bytes).unwrap();
    assert_eq!(report.vertices, 300_000);
    assert_eq!(report.max_image_dimension, 4096);
    assert_eq!(report.image_decoded_bytes, 4096 * 4096 * 4);
    assert!(report.warnings().is_empty());
    let report = glb::precheck(&textured_geometry(600_000, 8192, 33 * 1024 * 1024)).unwrap();
    assert_eq!(report.warnings().len(), 3);
    assert!(
        report
            .warnings()
            .iter()
            .any(|note| note.contains("vertices"))
    );
}

#[test]
fn byte_geometry_and_image_safety_limits_are_hard_rejections() {
    let bytes = vec![0; 300 * 1024 * 1024];
    assert!(
        glb::precheck(&bytes)
            .unwrap_err()
            .message
            .contains("GLB_BYTES")
    );
    drop(bytes);
    assert!(
        glb::precheck(&textured_geometry(3, 20_000, 0))
            .unwrap_err()
            .message
            .contains("IMAGE_DIMENSION")
    );
    let (mut document, binary) = common::triangle();
    // Repeated primitive references exceed the total without a large buffer allocation.
    document["meshes"][0]["primitives"] = json!(vec![json!({"attributes":{"POSITION":0}}); 6]);
    document["accessors"][0]["count"] = json!(3_000_000);
    document["bufferViews"][0]["byteLength"] = json!(36_000_000);
    document["buffers"][0]["byteLength"] = json!(36_000_000);
    let mut binary = binary;
    binary.resize(36_000_000, 0);
    assert!(
        glb::precheck(&common::glb(&document, &binary))
            .unwrap_err()
            .message
            .contains("VERTICES")
    );
    // One maximum-edge image decodes to a quarter of the budget; four reach it exactly.
    let bytes = textured_geometry(3, 16_384, 0);
    let json_len = u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize;
    let mut document: Value = serde_json::from_slice(&bytes[20..20 + json_len]).unwrap();
    let bin = &bytes[28 + json_len..];
    let image = json!({"bufferView":1,"mimeType":"image/png"});
    for _ in 0..3 {
        document["images"]
            .as_array_mut()
            .unwrap()
            .push(image.clone());
    }
    assert_eq!(
        glb::precheck(&common::glb(&document, bin))
            .unwrap()
            .image_decoded_bytes,
        limits::IMAGE_DECODED_BYTES
    );
    document["images"].as_array_mut().unwrap().push(image);
    assert!(
        glb::precheck(&common::glb(&document, bin))
            .unwrap_err()
            .message
            .contains("IMAGE_DECODED_BYTES")
    );
}

#[test]
fn decoded_image_reference_warns_without_rejecting() {
    // A single maximum-edge image stays inside the safety ceiling but passes the reference.
    let report = glb::precheck(&textured_geometry(3, 16_384, 0)).unwrap();
    assert!(report.image_decoded_bytes > limits::IMAGE_DECODED_WARN_BYTES);
    assert!(
        report
            .warnings()
            .iter()
            .any(|warning| warning.starts_with("decoded image bytes"))
    );
    // The former quality cap decodes to 64 MiB and stays below the reference.
    let report = glb::precheck(&textured_geometry(3, 4096, 0)).unwrap();
    assert!(
        !report
            .warnings()
            .iter()
            .any(|warning| warning.starts_with("decoded image bytes"))
    );
}
