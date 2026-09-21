#[path = "../../../tests/common/mod.rs"]
mod common;
use rou_core::{documents::Manifest, spatial::Profile, viewer};
use serde_json::json;

fn manifest() -> Manifest {
    let (world, records) = common::world();
    world.projections(&records).unwrap().manifest().unwrap()
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
        })
        .unwrap();
        assert_eq!(expected, index.query(position).unwrap());
        expected
    };
    // Both entities occupy 32 m chunks; the boundary is inclusive at exactly 48 m.
    assert_eq!(query([0., 80., 0.])["entity_ids"], json!(["house"]));
    assert_eq!(query([0., 80.001, 0.])["entity_ids"], json!([]));
    assert_eq!(
        query([32.5, 0.5, 0.5])["entity_ids"],
        json!(["bench", "house"])
    );
    assert_eq!(
        query([0.5, 0.5, 0.5])["entity_ids"],
        json!(["house", "bench"])
    );
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
