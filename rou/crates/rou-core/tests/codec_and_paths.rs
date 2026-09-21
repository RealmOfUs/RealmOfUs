use rou_core::{
    blender,
    documents::{Descriptor, Entity, Model, Realm},
    formats, json, lfs, limits, ops, paths,
    spatial::Profile,
};
use serde_json::json as value;

#[test]
fn strict_json_rejects_ambiguity_encodings_depth_and_overflow() {
    for raw in [
        b"{\"a\":1,\"a\":2}".as_slice(),
        br#"{"a":1,"\u0061":2}"#,
        b"[NaN]",
        b"[Infinity]",
        b"1e999",
        b"-1e999",
        b"\xef\xbb\xbf{}",
        b"\xff\xfe{\x00}\x00",
        b"\xff",
        br#""\ud800""#,
        b"{} {}",
    ] {
        assert!(json::strict(raw).is_err(), "{raw:?}");
    }
    let at_limit = format!("{}0{}", "[".repeat(40), "]".repeat(40));
    assert!(json::strict(at_limit.as_bytes()).is_ok());
    assert!(json::strict(format!("[{at_limit}]").as_bytes()).is_err());
    assert!(json::strict(&vec![b' '; limits::JSON_BYTES + 1]).is_err());
    assert!(json::strict(br#"{"a":{"a":1},"b":2}"#).is_ok());
}

#[test]
fn rfc8785_numbers_utf16_order_and_readable_output() {
    // RFC 8785 Section 3.2.2 example, independently specified expected bytes.
    let data = json::strict(
        br#"{"numbers":[333333333.33333329,1E30,4.50,2e-3,0.000000000000000000000000001]}"#,
    )
    .unwrap();
    assert_eq!(
        json::canonical(&data).unwrap(),
        br#"{"numbers":[333333333.3333333,1e+30,4.5,0.002,1e-27]}"#
    );
    assert_eq!(
        json::canonical(&value!([-0.0, 1.0, 1e20, 1e21, 1e-6, 1e-7])).unwrap(),
        b"[0,1,100000000000000000000,1e+21,0.000001,1e-7]"
    );
    let keys = value!({"\u{e000}": 2, "\u{1f600}": 1, "a": 0});
    assert_eq!(
        String::from_utf8(json::canonical(&keys).unwrap()).unwrap(),
        "{\"a\":0,\"\u{1f600}\":1,\"\u{e000}\":2}"
    );
    assert_eq!(
        json::pretty(&value!({"z": [1.0,-0.0], "a":"\u{5171}\u{5883}"})).unwrap(),
        "{\n  \"a\": \"\u{5171}\u{5883}\",\n  \"z\": [\n    1,\n    0\n  ]\n}\n".as_bytes()
    );
    assert_eq!(
        json::digest(b"abc"),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
}

#[test]
fn safe_paths_preserve_bytes_and_blender_relative_paths() {
    for path in [
        "../x",
        "/x",
        "a//b",
        "a/./b",
        "a/../b",
        ".git/config",
        ".GIT/config",
        ".Rou/x",
        "~/x",
        " x",
        "x ",
        "a/ b",
        "a\\b",
        "C:/x",
        "a\n",
    ] {
        assert!(paths::safe(path).is_err(), "{path}");
    }
    for path in [
        "sources/\u{623f}\u{5c4b}/\u{5ead}\u{9662}.blend",
        "sources/a b.png",
        "README.md",
    ] {
        paths::safe(path).unwrap();
    }
    paths::safe_bytes(b"sources/\xff.blend").unwrap();
    assert_eq!(
        blender::relative_path("sources/house/a.blend", "//../shared/lib.blend").unwrap(),
        "sources/shared/lib.blend"
    );
    assert!(blender::relative_path("sources/a.blend", "//../../secret").is_err());
    assert!(blender::relative_path("sources/a.blend", "/outside/a.png").is_err());
    for id in ["", "Upper", "with.dot", "\u{7a7a}", "a/b"] {
        assert!(paths::identity(id).is_err());
    }
}

#[test]
fn lfs_pointer_and_actual_source_bytes_have_one_integrity_rule() {
    let data = b"source-byte-validation-test";
    let descriptor = Descriptor::of(data);
    descriptor.verify(data).unwrap();
    assert!(descriptor.verify(&vec![b'x'; data.len()]).is_err());
    assert!(descriptor.verify(b"short").is_err());
    let pointer = lfs::encode(&descriptor).unwrap();
    assert_eq!(lfs::pointer(&pointer).unwrap(), Some(descriptor.clone()));
    assert_eq!(lfs::descriptor(&pointer).unwrap(), descriptor);
    assert_eq!(lfs::descriptor(data).unwrap(), descriptor);
    assert!(lfs::pointer(&[pointer.clone(), b"extra bad\n".to_vec()].concat()).is_err());
    for size in [
        "-1",
        "true",
        "1e2",
        "1073741825",
        "999999999999999999999999",
    ] {
        let pointer = format!(
            "{}\noid sha256:{}\nsize {size}\n",
            lfs::VERSION,
            descriptor.sha256
        );
        assert!(lfs::pointer(pointer.as_bytes()).is_err());
    }
    assert!(lfs::pointer(b"version https://git-lfs.github.com/spec/v2\n").is_err());
}

#[test]
fn new_shapes_reject_old_layout_and_unknown_fields() {
    let realm = value!({"format": formats::REALM, "id":"realm_test", "profile":Profile::default()});
    json::from_value::<Realm>(realm.clone()).unwrap();
    for field in ["entries", "viewpoints", "unknown"] {
        let mut changed = realm.clone();
        changed[field] = value!([]);
        assert!(json::from_value::<Realm>(changed).is_err());
    }
    let model = value!({"format":formats::MODEL,"id":"house","source":"sources/house.blend","entry":{"kind":"object","name":"House"},"dependencies":[],"settings":{}});
    json::from_value::<Model>(model.clone())
        .unwrap()
        .validate()
        .unwrap();
    let mut changed = model.clone();
    changed["settings"] = value!({"ignored":true});
    assert!(json::from_value::<Model>(changed).is_err());
    let mut changed = model.clone();
    changed["format"] = value!("rou-authoring/0.1");
    assert!(json::from_value::<Model>(changed).is_err());
    let mut changed = model.clone();
    changed["dependencies"] = value!(["sources/house.blend"]);
    assert!(
        json::from_value::<Model>(changed)
            .unwrap()
            .validate()
            .is_err()
    );
    let entity =
        value!({"format": formats::ENTITY, "id":"house", "model":"house", "position_mm":[1.0,0,0]});
    json::from_value::<Entity>(entity.clone()).unwrap();
    for changed in [
        value!({"format": formats::ENTITY, "id":"house", "binding":"house", "position_mm":[0,0,0]}),
        {
            let mut e = entity.clone();
            e["label"] = Value::Null;
            e
        },
    ] {
        assert!(json::from_value::<Entity>(changed).is_err());
    }
}

use serde_json::Value;

#[test]
fn placement_and_flat_geometry_regressions() {
    assert_eq!(
        blender::placement([1.0, 2.0, 3.0]).unwrap(),
        [1000, 3000, -2000]
    );
    assert_eq!(
        blender::inverse([1000, 3000, -2000]).unwrap(),
        [1.0, 2.0, 3.0]
    );
    assert_eq!(
        blender::placement([0.0005, -0.0005, -0.0005]).unwrap(),
        [1, -1, 1]
    );
    assert!(blender::placement([f64::NAN, 0.0, 0.0]).is_err());
    let bounds = blender::bounds(&[[-1.0, 0.0, -1.0], [1.0, 0.0, 1.0]]).unwrap();
    assert_eq!(bounds.min, [-1000, 0, -1000]);
    assert_eq!(bounds.max, [1000, 1, 1000]);
}

#[test]
fn operations_envelope_reports_stable_errors() {
    let (status, bytes) = ops::envelope(ops::dispatch("not.an.operation", b"{}"));
    assert_ne!(status, 0);
    assert_eq!(
        json::strict(&bytes).unwrap()["error"]["code"],
        "unknown_operation"
    );
    let (status, bytes) = ops::envelope(ops::dispatch("digest.sha256", b"abc"));
    assert_eq!(status, 0);
    assert_eq!(json::strict(&bytes).unwrap()["value"], json::digest(b"abc"));
}
