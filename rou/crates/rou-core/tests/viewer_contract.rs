#[path = "../../../tests/common/mod.rs"]
mod common;
use rou_core::{documents::Manifest, ops, spatial::Profile, viewer};
use serde_json::json;

fn manifest() -> Manifest {
    let (world, records) = common::world();
    world.projections(&records).unwrap().manifest().unwrap()
}

fn distinct_manifest() -> Manifest {
    let mut world = manifest();
    for entity in &mut world.entities {
        entity.lod1.sha256 = rou_core::json::digest(entity.id.as_bytes());
        entity.lod1.path = rou_core::paths::object(&entity.lod1.sha256);
    }
    world
}

#[test]
fn near_hysteresis_depends_on_the_mounted_sha_and_preserves_replacement_boxes() {
    let world = distinct_manifest();
    let house = world.entities.iter().find(|e| e.id == "house").unwrap();
    let index = viewer::Catalog::new(&world).unwrap();
    let mut request = viewer::Residency {
        position: [0., 49., 0.],
        radii: [16., 64., 4000.],
        loaded: Default::default(),
        hysteresis: viewer::default_hysteresis(),
    };
    let fresh = index.query(&request).unwrap();
    assert_eq!(fresh["tiers"]["near"], json!([]));
    request
        .loaded
        .insert("house".into(), house.asset.sha256.clone());
    let resident = index.query(&request).unwrap();
    assert_eq!(resident["tiers"]["near"], json!(["house"]));
    assert_eq!(resident["keep"], json!(["house"]));
    request.position[1] = 32. + 16. * request.hysteresis;
    assert_eq!(index.query(&request).unwrap()["keep"], json!(["house"]));
    request.position[1] += 0.001;
    let replacement = index.query(&request).unwrap();
    assert_eq!(replacement["load"], json!(["house", "bench"]));
    assert_eq!(replacement["keep"], json!([]));
    assert_eq!(replacement["unload"], json!([]));
    assert_eq!(replacement["impostor_hidden"], json!(["house"]));
    assert_eq!(replacement["impostor_visible"], json!(["bench"]));
    request
        .loaded
        .insert("house".into(), house.lod1.sha256.clone());
    request.position[1] = 49.;
    assert_eq!(index.query(&request).unwrap()["tiers"]["near"], json!([]));
    request.position[1] = 48.;
    let approach = index.query(&request).unwrap();
    assert_eq!(approach["tiers"]["near"], json!(["house"]));
    assert_eq!(approach["load"], json!(["house", "bench"]));
}

#[test]
fn replacements_share_nearest_first_work_order_with_new_loads() {
    let world = distinct_manifest();
    let house = world.entities.iter().find(|e| e.id == "house").unwrap();
    let index = viewer::Catalog::new(&world).unwrap();
    let mut request = viewer::Residency {
        position: [0.5; 3],
        radii: [16., 64., 4000.],
        loaded: [("house".into(), house.lod1.sha256.clone())].into(),
        hysteresis: viewer::default_hysteresis(),
    };
    let upgrade = index.query(&request).unwrap();
    assert_eq!(upgrade["load"], json!(["house", "bench"]));
    assert_eq!(upgrade["keep"], json!([]));
    assert_eq!(upgrade["unload"], json!([]));
    // At an exact AABB-center tie, IDs still break ties across both work kinds.
    request.position[0] = 16.5;
    assert_eq!(
        index.query(&request).unwrap()["load"],
        json!(["bench", "house"])
    );
    request
        .loaded
        .insert("house".into(), house.asset.sha256.clone());
    request.position = [0., 100., 0.];
    let retained_swap = index.query(&request).unwrap();
    assert_eq!(retained_swap["load"], json!(["house"]));
    assert_eq!(retained_swap["keep"], json!([]));
    assert_eq!(retained_swap["impostor_hidden"], json!(["house"]));
    request.loaded.clear();
    assert_eq!(index.query(&request).unwrap()["load"], json!([]));
}

#[test]
fn identical_tier_hashes_use_full_hysteresis_without_replacement_work() {
    let world = manifest();
    let index = viewer::Catalog::new(&world).unwrap();
    let mut request = viewer::Residency {
        position: [0., 49., 0.],
        radii: [16., 64., 4000.],
        loaded: [("house".into(), world.entities[0].asset.sha256.clone())].into(),
        hysteresis: viewer::default_hysteresis(),
    };
    assert_eq!(
        index.query(&request).unwrap()["tiers"]["near"],
        json!(["house"])
    );
    request.position[1] = 51.;
    let mid = index.query(&request).unwrap();
    assert_eq!(mid["keep"], json!(["house"]));
    assert_eq!(mid["load"], json!(["bench"]));
    assert_eq!(mid["tiers"]["near"], json!([]));
    assert!(
        mid["tiers"]["mid"]
            .as_array()
            .unwrap()
            .contains(&json!("house"))
    );
}

#[test]
fn default_camera_frames_the_union_and_empty_world() {
    let mut world = manifest();
    let pose = viewer::default_pose(&world).unwrap();
    assert_eq!(pose.look_at, [16.5, 0.5, 0.5]);
    assert_eq!(pose.position, [16.5, 0.5 + 1091_f64.sqrt(), 0.5]);
    assert!(viewer::pose(pose).is_ok());
    world.entities.clear();
    world.chunks.clear();
    let pose = viewer::default_pose(&world).unwrap();
    assert_eq!(pose.position, [0., 2., 0.]);
    assert_eq!(pose.look_at, [0.; 3]);
}

#[test]
fn streaming_uses_distance_to_chunks_deduplicates_and_orders_nearest() {
    let world = manifest();
    let index = viewer::Catalog::new(&world).unwrap();
    let query = |position| {
        let expected = viewer::query(&viewer::Query {
            manifest: world.clone(),
            position,
            radii: [16., 48., 4000.],
            loaded: Default::default(),
            hysteresis: viewer::default_hysteresis(),
        })
        .unwrap();
        assert_eq!(
            expected,
            index
                .query(&viewer::Residency {
                    position,
                    radii: [16., 48., 4000.],
                    loaded: Default::default(),
                    hysteresis: viewer::default_hysteresis(),
                })
                .unwrap()
        );
        expected
    };
    // Both entities occupy 32 m chunks; the boundary is inclusive at exactly 48 m.
    assert_eq!(query([0., 80., 0.])["load"], json!(["house"]));
    assert_eq!(query([0., 80.001, 0.])["load"], json!([]));
    assert_eq!(query([32.5, 0.5, 0.5])["load"], json!(["bench", "house"]));
    assert_eq!(query([0.5, 0.5, 0.5])["load"], json!(["house", "bench"]));
}

#[test]
fn metre_input_and_camera_poses_are_finite_and_allow_overview_altitude() {
    assert_eq!(viewer::at("-1.25, 2.5,0").unwrap(), [-1.25, 2.5, 0.]);
    for invalid in [
        "1,2",
        "1,2,3,4",
        "NaN,2,3",
        "1,inf,3",
        "1e1000,2,3",
        "4000001,0,0",
    ] {
        assert!(viewer::at(invalid).is_err(), "{invalid}");
    }
    assert!(
        viewer::pose(viewer::Pose {
            position: [0., 3_000_000., 0.],
            look_at: [0.; 3]
        })
        .is_ok()
    );
    assert!(
        viewer::pose(viewer::Pose {
            position: [0.; 3],
            look_at: [0.; 3]
        })
        .is_err()
    );
    let aimed = viewer::aim(viewer::Pose {
        position: [0.; 3],
        look_at: [0.; 3],
    })
    .unwrap();
    assert_eq!(aimed.look_at, [0., 0., -1.]);
    let p = Profile::default();
    assert_eq!(viewer::chunk_at(&p, [-0.001, 0., 0.]).unwrap(), [-1, 0, 0]);
    assert_eq!(viewer::chunk_center(&p, [0; 3]).unwrap(), [16.; 3]);
    assert!(viewer::chunk_center(&p, p.max_chunk).is_err());
}

#[test]
fn map_and_catalog_preserve_known_empty_cells_at_world_edges() {
    let world = manifest();
    let map = viewer::map(&viewer::MapRequest {
        profile: world.profile.clone(),
        position: [0., 1000., 0.],
        layer: None,
    })
    .unwrap();
    assert_eq!(map["layer"], world.profile.max_chunk[1] - 1);
    for cell in map["cells"].as_array().unwrap() {
        let chunk = serde_json::from_value(cell["chunk"].clone()).unwrap();
        assert!(viewer::chunk_center(&world.profile, chunk).is_ok());
    }
    let catalog = viewer::catalog(&world).unwrap();
    assert_eq!(catalog["known"]["-1,0,0"], json!([]));
    assert_eq!(
        catalog["entities"]["bench"]["position"],
        json!([32., 0., 0.])
    );
}

#[test]
fn tiers_hysteresis_and_missing_payloads_use_only_catalog_metadata() {
    let index = viewer::Catalog::new(&manifest()).unwrap();
    let mut request = viewer::Residency {
        position: [0., 48., 0.],
        radii: [16., 64., 4000.],
        loaded: Default::default(),
        hysteresis: viewer::default_hysteresis(),
    };
    let near = index.query(&request).unwrap();
    assert_eq!(near["tiers"]["near"], json!(["house"]));
    assert_eq!(near["tiers"]["mid"], json!(["bench"]));
    assert_eq!(near["load"], json!(["house", "bench"]));
    assert_eq!(near["impostor_hidden"], json!([]));
    request.position[1] = 48.001;
    assert_eq!(index.query(&request).unwrap()["tiers"]["near"], json!([]));
    request.position[1] = 96.;
    assert_eq!(index.query(&request).unwrap()["load"], json!(["house"]));
    request.position[1] = 96.001;
    assert_eq!(index.query(&request).unwrap()["load"], json!([]));
    request.loaded = [("house".into(), manifest().entities[0].asset.sha256.clone())].into();
    request.position[1] = 32. + 64. * request.hysteresis;
    let retained = index.query(&request).unwrap();
    assert_eq!(retained["keep"], json!(["house"]));
    assert_eq!(retained["impostor_hidden"], retained["keep"]);
    assert_eq!(retained["impostor_visible"], json!(["bench"]));
    assert_eq!(retained["unload"], json!([]));
    request.position[1] += 0.001;
    let released = index.query(&request).unwrap();
    assert_eq!(released["unload"], json!(["house"]));
    assert_eq!(released["impostor_visible"], json!(["house", "bench"]));
    request.loaded.clear();
    request.position[1] = 4032.;
    assert_eq!(
        index.query(&request).unwrap()["tiers"]["far"],
        json!(["house"])
    );
    request.position[1] += 0.001;
    assert_eq!(
        index.query(&request).unwrap()["impostor_visible"],
        json!([])
    );
    // No filesystem is consulted: even a placeholder asset must still be requested.
    request.position = [0., 120., 0.];
    assert!(
        index.query(&request).unwrap()["load"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    request.radii = viewer::default_radii();
    assert_eq!(
        index.query(&request).unwrap()["load"],
        json!(["house", "bench"])
    );
}

#[test]
fn streaming_validates_settings_and_loaded_ids_and_supplies_impostors() {
    let world = manifest();
    let layout = viewer::catalog(&world).unwrap();
    let house = layout["impostors"]
        .as_array()
        .unwrap()
        .iter()
        .find(|v| v["id"] == "house")
        .unwrap();
    assert_eq!(house["center_m"], json!([0.5, 0.5, 0.5]));
    assert_eq!(house["size_m"], json!([1., 1., 1.]));
    for radii in [
        [0., 64., 4000.],
        [-1., 64., 4000.],
        [65., 64., 4000.],
        [16., 64., f64::INFINITY],
        [16., 64., 4_000_001.],
    ] {
        assert!(viewer::radii(radii).is_err());
    }
    let index = viewer::Catalog::new(&world).unwrap();
    let mut request = viewer::Residency {
        position: [0.; 3],
        radii: viewer::default_radii(),
        loaded: Default::default(),
        hysteresis: 1.,
    };
    for bad in [0.99, f64::NAN, f64::INFINITY, 100_000.] {
        request.hysteresis = bad;
        assert!(index.query(&request).is_err());
    }
    request.hysteresis = 1.;
    request.loaded = [("absent".into(), "0".repeat(64))].into();
    assert!(index.query(&request).is_err());
    request.loaded.clear();
    request.radii = [rou_core::limits::VIEW_COORDINATE_METRES; 3];
    request.hysteresis = viewer::default_hysteresis();
    assert!(index.query(&request).is_ok());
    let parsed: viewer::Residency = serde_json::from_value(json!({
        "position": [0,0,0], "radii": [16,64,4000]
    }))
    .unwrap();
    assert_eq!(parsed.hysteresis, 1.15);
    assert!(
        serde_json::from_value::<viewer::Residency>(json!({
            "position": [0,0,0], "radii": [16,64,4000], "typo": true
        }))
        .is_err()
    );
}

#[test]
fn equal_radii_prioritize_nearer_tiers_and_retention_can_exceed_far_radius() {
    let index = viewer::Catalog::new(&manifest()).unwrap();
    let mut request = viewer::Residency {
        position: [0., 48., 0.],
        radii: [16.; 3],
        loaded: Default::default(),
        hysteresis: 1.15,
    };
    let boundary = index.query(&request).unwrap();
    assert_eq!(boundary["tiers"]["near"], json!(["house"]));
    assert_eq!(boundary["tiers"]["mid"], json!([]));
    assert_eq!(boundary["tiers"]["far"], json!([]));
    request.radii[0] = 8.;
    assert_eq!(
        index.query(&request).unwrap()["tiers"]["mid"],
        json!(["house"])
    );
    request.position[1] = 49.;
    request.loaded = [("house".into(), manifest().entities[0].asset.sha256.clone())].into();
    let retained = index.query(&request).unwrap();
    assert_eq!(retained["keep"], json!(["house"]));
    assert_eq!(retained["tiers"]["mid"], json!(["house"]));
    assert_eq!(retained["impostor_hidden"], json!(["house"]));
    assert_eq!(retained["impostor_visible"], json!([]));
    request.position[1] = 51.;
    let unloaded = index.query(&request).unwrap();
    assert_eq!(unloaded["unload"], json!(["house"]));
    assert_eq!(unloaded["impostor_visible"], json!([]));
}

#[test]
fn retention_has_an_explicit_distance_budget_independent_of_the_default() {
    let index = viewer::Catalog::new(&manifest()).unwrap();
    let mut request = viewer::Residency {
        position: [0.; 3],
        radii: [4_000_000.; 3],
        loaded: Default::default(),
        hysteresis: 1.15,
    };
    assert!(index.query(&request).is_ok());
    request.hysteresis = 1.16;
    assert!(index.query(&request).is_err());
    request.radii = [3_000_000.; 3];
    assert!(index.query(&request).is_ok());
    let config: serde_json::Value =
        serde_json::from_slice(&ops::dispatch("config", b"").unwrap()).unwrap();
    assert_eq!(config["viewer"]["max_retention"], 4_600_000.);
    assert_eq!(config["viewer"]["max_radius"], 4_000_000.);
}

#[test]
fn viewer_dispatch_rejects_bad_radii_queries_and_loaded_ids() {
    for invalid in [
        json!([0, 16, 64]),
        json!([64, 16, 64]),
        json!([16, 64]),
        json!([16, 64, null]),
    ] {
        assert!(ops::dispatch("viewer.radii", &serde_json::to_vec(&invalid).unwrap()).is_err());
    }
    let valid = json!({"manifest": manifest(), "position": [0, 0, 0], "radii": [16, 64, 4000]});
    assert!(ops::dispatch("viewer.query", &serde_json::to_vec(&valid).unwrap()).is_ok());
    for changes in [
        json!({"radii": null}),
        json!({"radii": [64, 16, 4000]}),
        json!({"loaded": ["house"]}),
        json!({"loaded": {"absent": "0".repeat(64)}}),
        json!({"loaded": {"house": "0".repeat(64)}}),
        json!({"hysteresis": 0.5}),
        json!({"unexpected": true}),
    ] {
        let mut request = valid.clone();
        for (key, value) in changes.as_object().unwrap() {
            request[key] = value.clone();
        }
        assert!(ops::dispatch("viewer.query", &serde_json::to_vec(&request).unwrap()).is_err());
    }
    let mut missing = valid;
    missing.as_object_mut().unwrap().remove("radii");
    assert!(ops::dispatch("viewer.query", &serde_json::to_vec(&missing).unwrap()).is_err());
}
