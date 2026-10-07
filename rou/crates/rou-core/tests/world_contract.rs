#[path = "../../../tests/common/mod.rs"]
mod common;
use rou_core::{documents::*, formats, json, paths, world};
use serde_json::{Value, json as value};

#[test]
fn spatial_selection_expands_shared_dependencies_and_keeps_runtime_cone() {
    use rou_core::selection::Selection;
    let (world, records) = common::world();
    let view = world.projections(&records).unwrap();
    let selected = Selection {
        chunks: vec!["0,0,0".into()],
        ..Default::default()
    }
    .resolve(&world, &view)
    .unwrap();
    assert_eq!(selected.entity_ids, ["house"]);
    assert_eq!(
        selected.paths,
        ["sources/house/house.blend", "sources/shared/wood.png"]
    );
    assert_eq!(
        selected.assets,
        ["runtime/house.glb", "runtime/house.lod1.glb"]
    );
    assert_eq!(
        selected.directories,
        [
            "docs",
            "realm",
            "runtime",
            "sources/house",
            "sources/shared"
        ]
    );
    assert!(
        Selection {
            all: true,
            chunks: vec!["0,0,0".into()],
            ..Default::default()
        }
        .validate()
        .is_err()
    );
    assert!(
        Selection {
            entities: vec!["missing".into()],
            ..Default::default()
        }
        .resolve(&world, &view)
        .is_err()
    );
    let empty = json::pretty(&Selection::default()).unwrap();
    assert_eq!(
        json::strict(&empty).unwrap(),
        value!({"all": false, "chunks": [], "entities": []})
    );
}
use std::collections::BTreeMap;

#[test]
fn duplicate_source_entries_are_rejected_but_shared_model_entities_are_valid() {
    let (mut world, _) = common::world();
    let original = world.models["house"].clone();
    let duplicate = world.models.get_mut("bench").unwrap();
    duplicate.source = original.source;
    duplicate.entry = original.entry;
    assert_eq!(
        world.validate_metadata().unwrap_err().code,
        "duplicate_entry"
    );
    world.models.remove("bench");
    world.entities.get_mut("bench").unwrap().model = "house".into();
    world.validate_metadata().unwrap();
}

#[test]
fn empty_world_needs_no_build_or_viewing_entry() {
    let (mut world, _) = common::world();
    world.models.clear();
    world.entities.clear();
    world.inputs.clear();
    let view = world.projections(&BTreeMap::new()).unwrap();
    assert!(view.chunks.is_empty());
    let manifest = view.manifest().unwrap();
    manifest.validate().unwrap();
    let value = serde_json::to_value(manifest).unwrap();
    assert!(value.get("entries").is_none());
    assert!(value.get("viewpoints").is_none());
}

#[test]
fn placement_changes_projection_without_rebuilding_geometry() {
    let (mut world, records) = common::world();
    let old_digest = world.input_digest("house").unwrap();
    let before = world.projections(&records).unwrap();
    world.entities.get_mut("house").unwrap().position_mm = [64000, 0, 0];
    let after = world.projections(&records).unwrap();
    assert_eq!(old_digest, world.input_digest("house").unwrap());
    assert_ne!(before.root_hash().unwrap(), after.root_hash().unwrap());
    let house = after.entities.iter().find(|e| e.id == "house").unwrap();
    assert_eq!(house.bounds_mm.min, [64000, 0, 0]);
    assert_eq!(house.owner_chunk, [2, 0, 0]);
    assert_eq!(house.asset, records["house"].asset);
    assert_eq!(after.chunks["2,0,0"], ["house"]);
    let manifest = after.manifest().unwrap();
    assert_eq!(
        manifest
            .entities
            .iter()
            .find(|e| e.id == "house")
            .unwrap()
            .asset
            .path,
        paths::object(&house.asset.sha256)
    );
}

#[test]
fn changed_shared_dependency_invalidates_every_affected_model() {
    let (mut world, records) = common::world();
    for inputs in world.inputs.values_mut() {
        inputs.insert("sources/shared/wood.png".into(), Descriptor::of(b"changed"));
    }
    for (id, record) in records {
        assert!(
            record
                .validate(&world.realm.profile, &world.models[&id], &world.inputs[&id])
                .is_err()
        );
    }
}

#[test]
fn input_digest_uses_spec_fields_and_excludes_outputs_and_tool_versions() {
    let (world, mut records) = common::world();
    let expected = value!({"exporter":formats::BLENDER_EXPORT,"profile":world.realm.profile,"model":world.models["house"],"inputs":world.inputs["house"]});
    assert_eq!(
        world.input_digest("house").unwrap(),
        json::hash(&expected).unwrap()
    );
    records.get_mut("house").unwrap().tool_versions =
        Some([("blender".into(), "different version".into())].into());
    world.projections(&records).unwrap();
    records.get_mut("house").unwrap().input_digest = "0".repeat(64);
    assert!(world.projections(&records).is_err());
}

#[test]
fn references_ids_records_and_dependency_cycles_are_checked() {
    let (original, records) = common::world();
    world::record_id("realm/models/house.json", paths::MODELS, "house").unwrap();
    assert!(world::record_id("realm/models/other/house.json", paths::MODELS, "house").is_err());
    let mut world = original.clone();
    world.entities.get_mut("house").unwrap().model = "missing".into();
    assert!(world.validate().is_err());
    let mut world = original.clone();
    world.entities.get_mut("house").unwrap().id = "duplicate".into();
    assert!(world.validate().is_err());
    let mut world = original.clone();
    world
        .inputs
        .get_mut("house")
        .unwrap()
        .remove("sources/shared/wood.png");
    assert!(world.validate().is_err());
    let mut world = original.clone();
    for (id, other) in [("house", "bench"), ("bench", "house")] {
        let source = world.models[other].source.clone();
        world
            .models
            .get_mut(id)
            .unwrap()
            .dependencies
            .push(source.clone());
        world
            .inputs
            .get_mut(id)
            .unwrap()
            .insert(source, Descriptor::of(b"library"));
    }
    assert_eq!(world.validate().unwrap_err().code, "dependency_cycle");
    let mut missing = records.clone();
    missing.remove("house");
    assert!(original.projections(&missing).is_err());
    let mut extra = records.clone();
    extra.insert("orphan".into(), records["house"].clone());
    assert!(original.projections(&extra).is_err());
}

#[test]
fn shared_model_instances_and_source_scoped_session_baseline() {
    let (mut world, records) = common::world();
    let old = world::session_fingerprint(&world, "sources/house/house.blend").unwrap();
    world.entities.get_mut("bench").unwrap().position_mm = [40000, 0, 0];
    assert_eq!(
        old,
        world::session_fingerprint(&world, "sources/house/house.blend").unwrap()
    );
    let mut second = world.entities["house"].clone();
    second.id = "house_two".into();
    second.position_mm = [10000, 0, 0];
    world.entities.insert(second.id.clone(), second);
    let view = world.projections(&records).unwrap();
    let houses: Vec<_> = view
        .entities
        .iter()
        .filter(|e| e.id.starts_with("house"))
        .collect();
    assert_eq!(houses.len(), 2);
    assert_eq!(houses[0].asset, houses[1].asset);
    assert_ne!(
        old,
        world::session_fingerprint(&world, "sources/house/house.blend").unwrap()
    );
}

#[test]
fn diff_reports_placement_geometry_addition_and_removal() {
    let (before, _) = common::world();
    let mut after = before.clone();
    after.entities.get_mut("house").unwrap().position_mm = [1000, 0, 0];
    for inputs in after.inputs.values_mut() {
        inputs.insert(
            "sources/shared/wood.png".into(),
            Descriptor::of(b"new texture"),
        );
    }
    let result = world::diff(Some(&before), Some(&after)).unwrap();
    let changes: BTreeMap<_, _> = result["changed_entities"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| (e["id"].as_str().unwrap(), e["changes"].clone()))
        .collect();
    assert_eq!(changes["house"], value!(["placement", "geometry"]));
    assert_eq!(changes["bench"], value!(["geometry"]));
    assert_eq!(
        world::diff(None, Some(&before)).unwrap()["added_entities"],
        value!(["bench", "house"])
    );
    after.entities.remove("house");
    assert_eq!(
        world::diff(Some(&before), Some(&after)).unwrap()["removed_entities"],
        value!(["house"])
    );
}

#[test]
fn lineage_and_version_sidecar_cannot_be_silently_rebound() {
    let (world, records) = common::world();
    let mut changed = world.realm.clone();
    changed.id = "other".into();
    assert!(world::lineage(&world.realm, &changed).is_err());
    let view = world.projections(&records).unwrap();
    let catalog = json::pretty(&view.manifest().unwrap()).unwrap();
    let version = Version {
        format: formats::GIT_VIEW.into(),
        realm_id: world.realm.id.clone(),
        git_object_format: "sha1".into(),
        commit_oid: "a".repeat(40),
        view_root_sha256: view.root_hash().unwrap(),
        catalog_sha256: json::digest(&catalog),
        validation: "local-trusted".into(),
    };
    version.validate(&catalog).unwrap();
    assert!(
        version
            .validate(&[catalog.clone(), b" ".to_vec()].concat())
            .is_err()
    );
    let mut invalid = serde_json::to_value(&version).unwrap();
    invalid["commit_oid"] = Value::String("a".repeat(64));
    assert!(json::from_value::<Version>(invalid).is_err());
    let mut draft = version;
    draft.commit_oid.clear();
    draft.validate(&catalog).unwrap();
}

#[test]
fn source_session_ignores_unrelated_metadata_but_validates_its_own_records() {
    let (mut world, _) = common::world();
    world.inputs.remove("bench");
    assert!(world.validate_metadata().is_ok());
    assert!(world::session_fingerprint(&world, "sources/house/house.blend").is_ok());
    let before = world::session_fingerprint(&world, "sources/house/house.blend").unwrap();
    world.entities.get_mut("bench").unwrap().model = "absent".into();
    assert_eq!(
        world::session_fingerprint(&world, "sources/house/house.blend").unwrap(),
        before
    );
    world.models.get_mut("house").unwrap().entry.name.clear();
    assert!(world::session_fingerprint(&world, "sources/house/house.blend").is_err());
}

#[test]
fn author_inputs_cannot_reference_runtime_outputs() {
    let (world, _) = common::world();
    let mut model = world.models["house"].clone();
    model.dependencies.push("runtime/house.glb".into());
    assert_eq!(model.validate().unwrap_err().code, "invalid_dependency");
    model.dependencies.pop();
    model.source = "runtime/self.blend".into();
    assert_eq!(model.validate().unwrap_err().code, "invalid_source");
}

#[test]
fn lod1_is_mandatory_and_versions_and_paths_are_strict() {
    let (world, records) = common::world();
    let mut record = serde_json::to_value(&records["house"]).unwrap();
    record.as_object_mut().unwrap().remove("lod1");
    assert!(json::from_value::<rou_core::documents::Build>(record).is_err());
    let mut record = records["house"].clone();
    record.format = "rou-build/0.1".into();
    assert!(record.validate_shape().is_err());
    record.format = rou_core::formats::BUILD.into();
    record.lod1.path = "runtime/other.lod1.glb".into();
    assert!(record.validate_shape().is_err());
    let manifest = world.projections(&records).unwrap().manifest().unwrap();
    assert_eq!(
        manifest.entities[0].asset.sha256,
        manifest.entities[0].lod1.sha256
    );
    let mut value = serde_json::to_value(&manifest).unwrap();
    value["entities"][0].as_object_mut().unwrap().remove("lod1");
    assert!(json::from_value::<rou_core::documents::Manifest>(value).is_err());
    let mut old = manifest.clone();
    old.format = "rou-preview/0.1".into();
    assert!(old.validate().is_err());
    let mut inconsistent = manifest;
    inconsistent.entities[0].lod1.bytes += 1;
    assert_eq!(inconsistent.validate().unwrap_err().code, "length_mismatch");
}

#[test]
fn retired_formats_report_regeneration_without_accepting_legacy_documents() {
    let (world, records) = common::world();
    let mut build = serde_json::to_value(&records["house"]).unwrap();
    build["format"] = value!("rou-build/0.1");
    build.as_object_mut().unwrap().remove("lod1");
    let mut manifest =
        serde_json::to_value(world.projections(&records).unwrap().manifest().unwrap()).unwrap();
    manifest["format"] = value!("rou-preview/0.1");
    for entity in manifest["entities"].as_array_mut().unwrap() {
        entity.as_object_mut().unwrap().remove("lod1");
    }
    let export = value!({"format":"rou-blender-export/0.1","models":[{
        "id":"house","asset_path":"runtime/house.glb",
        "bounds_blender_m":[[0,0,0],[1,1,1]],"dependencies":[]
    }],"tool_versions":{"exporter":"rou-blender-export/0.1"}});
    for (error, current) in [
        (
            json::from_value::<Build>(build.clone()).unwrap_err(),
            formats::BUILD,
        ),
        (
            json::from_value::<Manifest>(manifest).unwrap_err(),
            formats::PREVIEW,
        ),
        (
            json::from_value::<BlenderExport>(export).unwrap_err(),
            formats::BLENDER_EXPORT,
        ),
    ] {
        assert_eq!(error.code, "invalid_json");
        assert!(
            error
                .message
                .contains(&format!("document predates {current}"))
        );
        assert!(error.message.contains("regenerate with rou build"));
    }
    for format in [formats::BUILD, "rou-build/0.99"] {
        build["format"] = value!(format);
        let error = json::from_value::<Build>(build.clone()).unwrap_err();
        assert!(error.message.contains("missing field `lod1`"));
        assert!(!error.message.contains("predates"));
    }
    for bytes in [
        br#"{"format":"rou-build/0.1","broken":}"#.as_slice(),
        br#"{"format":"rou-build/0.1","format":"rou-build/0.2"}"#.as_slice(),
    ] {
        let error = json::parse::<Build>(bytes).unwrap_err();
        assert_eq!(error.code, "invalid_json");
        assert!(!error.message.contains("predates"));
    }
}
