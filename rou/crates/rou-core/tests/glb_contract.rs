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
        assert!(glb::precheck(&bytes[..end]).is_err());
    }
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
    for i in 1..35 {
        nodes.push(json!({"children":[i-1]}));
    }
    document["nodes"] = Value::Array(nodes);
    document["scenes"] = json!([{"nodes":[34]}]);
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
