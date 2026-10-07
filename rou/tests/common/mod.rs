#![allow(dead_code)]
use rou_core::{
    documents::*,
    formats, json, paths,
    spatial::{Bounds, Profile},
    world::World,
};
use serde_json::{Value, json};
use std::collections::BTreeMap;

pub fn triangle() -> (Value, Vec<u8>) {
    let binary = [0_f32, 0., 0., 1., 0., 0., 0., 1., 0.]
        .into_iter()
        .flat_map(f32::to_le_bytes)
        .collect();
    (
        json!({"asset":{"version":"2.0"},"buffers":[{"byteLength":36}],"bufferViews":[{"buffer":0,"byteLength":36}],
        "accessors":[{"bufferView":0,"componentType":5126,"count":3,"type":"VEC3"}],
        "meshes":[{"primitives":[{"attributes":{"POSITION":0}}]}],"nodes":[{"mesh":0}],"scenes":[{"nodes":[0]}],"scene":0}),
        binary,
    )
}
pub fn triangle_lod1() -> (Value, Vec<u8>) {
    let (mut document, mut binary) = triangle();
    document["extras"] = json!({"detail":"lod1"});
    binary[12..16].copy_from_slice(&0.5_f32.to_le_bytes());
    binary[28..32].copy_from_slice(&0.5_f32.to_le_bytes());
    (document, binary)
}
pub fn glb(document: &Value, binary: &[u8]) -> Vec<u8> {
    let mut text = json::canonical(document).unwrap();
    while text.len() % 4 != 0 {
        text.push(b' ');
    }
    let mut bin = binary.to_vec();
    while bin.len() % 4 != 0 {
        bin.push(0);
    }
    let mut result = Vec::new();
    for word in [
        0x46546c67_u32,
        2,
        (28 + text.len() + bin.len()) as u32,
        text.len() as u32,
        0x4e4f534a,
    ] {
        result.extend(word.to_le_bytes());
    }
    result.extend(text);
    result.extend((bin.len() as u32).to_le_bytes());
    result.extend(0x004e4942_u32.to_le_bytes());
    result.extend(bin);
    result
}
pub fn world() -> (World, BTreeMap<String, Build>) {
    let realm = Realm {
        format: formats::REALM.into(),
        id: "realm_test".into(),
        profile: Profile::default(),
    };
    let mut world = World {
        realm,
        models: BTreeMap::new(),
        entities: BTreeMap::new(),
        inputs: BTreeMap::new(),
    };
    let mut records = BTreeMap::new();
    let (document, binary) = triangle();
    let asset_bytes = glb(&document, &binary);
    for (id, position) in [("house", [0, 0, 0]), ("bench", [32000, 0, 0])] {
        let model = Model {
            format: formats::MODEL.into(),
            id: id.into(),
            source: format!("sources/{id}/{id}.blend"),
            entry: Entry {
                kind: EntryKind::Object,
                name: id.into(),
            },
            dependencies: vec!["sources/shared/wood.png".into()],
            settings: BTreeMap::new(),
        };
        let inputs: Inputs = [
            (model.source.clone(), Descriptor::of(id.as_bytes())),
            (model.dependencies[0].clone(), Descriptor::of(b"texture")),
        ]
        .into();
        records.insert(
            id.into(),
            Build {
                format: formats::BUILD.into(),
                model: id.into(),
                input_digest: input_digest(&world.realm.profile, &model, &inputs).unwrap(),
                inputs: inputs.clone(),
                asset: Asset {
                    path: paths::asset(id),
                    sha256: json::digest(&asset_bytes),
                    bytes: asset_bytes.len() as u64,
                },
                lod1: Asset {
                    path: paths::lod1(id),
                    sha256: json::digest(&asset_bytes),
                    bytes: asset_bytes.len() as u64,
                },
                local_bounds_mm: Bounds {
                    min: [0; 3],
                    max: [1000; 3],
                },
                tool_versions: None,
            },
        );
        world.inputs.insert(id.into(), inputs);
        world.models.insert(id.into(), model);
        world.entities.insert(
            id.into(),
            Entity {
                format: formats::ENTITY.into(),
                id: id.into(),
                model: id.into(),
                position_mm: position,
                label: None,
            },
        );
    }
    (world, records)
}
