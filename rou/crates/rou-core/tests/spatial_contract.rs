//! Reviewed spatial vectors remain unchanged; the harness supplies current asset fields.
use rou_core::{
    documents::Manifest,
    json, ops,
    spatial::{Bounds, Profile},
};
use serde_json::{Value, json as value};

fn fixtures() -> Value {
    let mut fixture = json::strict(include_bytes!("fixtures/spatial.json")).unwrap();
    fn adapt(value: &mut Value) {
        match value {
            Value::Object(map) => {
                if map.get("format") == Some(&value!("rou-preview/0.1")) {
                    map.insert("format".into(), value!(rou_core::formats::PREVIEW));
                }
                if let Some(asset) = map
                    .get("asset")
                    .filter(|asset| asset.get("path").is_some())
                    .cloned()
                {
                    map.insert("lod1".into(), asset);
                }
                for child in map.values_mut() {
                    adapt(child);
                }
            }
            Value::Array(items) => {
                for child in items {
                    adapt(child);
                }
            }
            _ => {}
        }
    }
    adapt(&mut fixture);
    fixture
}
fn changed(base: &Value, case: &Value, field: &str) -> Value {
    let mut value = base.clone();
    if let Some(changes) = case.get(field).and_then(Value::as_object) {
        for (key, item) in changes {
            value[key] = item.clone();
        }
    }
    if let Some(removed) = case.get("remove").and_then(Value::as_array) {
        for key in removed {
            value.as_object_mut().unwrap().remove(key.as_str().unwrap());
        }
    }
    value
}
fn call(op: &str, input: &Value) -> rou_core::Result<Value> {
    json::strict(&ops::dispatch(op, &serde_json::to_vec(input).unwrap())?)
}

#[test]
fn reviewed_profiles_shape_semantics_and_hashes() {
    let fixture = fixtures();
    assert_eq!(
        serde_json::to_value(Profile::default()).unwrap(),
        fixture["profile"]
    );
    assert_eq!(
        json::hash(&Profile::default()).unwrap(),
        fixture["profile_hash"]
    );
    for case in fixture["profiles"].as_array().unwrap() {
        let value = changed(&fixture["profile"], case, "changes");
        let shape = json::from_value::<Profile>(value.clone());
        assert_eq!(
            shape.is_ok(),
            case["shape_accepted"],
            "shape {}: {shape:?}",
            case["name"]
        );
        let valid = shape.and_then(|p| p.validate().map(|()| p));
        assert_eq!(
            valid.is_ok(),
            case["accepted"],
            "semantics {}: {valid:?}",
            case["name"]
        );
        assert_eq!(call("profile.validate", &value).is_ok(), case["accepted"]);
        if let Some(hash) = case.get("profile_hash") {
            assert_eq!(json::hash(&valid.unwrap()).unwrap(), hash.as_str().unwrap());
        }
    }
}

#[test]
fn reviewed_positions() {
    let fixture = fixtures();
    for case in fixture["positions"].as_array().unwrap() {
        let profile = changed(&fixture["profile"], case, "profile_changes");
        let result = call(
            "spatial.owner",
            &value!({"profile": profile, "position_mm": case["position_mm"]}),
        );
        assert_eq!(
            result.is_ok(),
            case["accepted"],
            "{}: {result:?}",
            case["name"]
        );
        if let Ok(value) = result {
            assert_eq!(
                value,
                value!({"chunk": case["chunk"], "offset": case["offset"]})
            );
        }
    }
}

#[test]
fn reviewed_half_open_bounds() {
    let fixture = fixtures();
    for case in fixture["bounds"].as_array().unwrap() {
        let profile = changed(&fixture["profile"], case, "profile_changes");
        let result = call(
            "spatial.intersections",
            &value!({"profile": profile, "bounds_mm": {"min": case["min"], "max": case["max"]}}),
        );
        assert_eq!(
            result.is_ok(),
            case["accepted"],
            "{}: {result:?}",
            case["name"]
        );
        if let Ok(value) = result {
            let expected = case.get("chunks").cloned().unwrap_or_else(|| {
                value!(
                    (0..case["chunk_count"].as_u64().unwrap())
                        .map(|x| [x, 0, 0])
                        .collect::<Vec<_>>()
                )
            });
            assert_eq!(value, expected, "{}", case["name"]);
        }
    }
}

#[test]
fn reviewed_complete_catalogs() {
    let fixture = fixtures();
    for case in fixture["catalogs"].as_array().unwrap() {
        let value = changed(&fixture["catalog"], case, "changes");
        let result = json::from_value::<Manifest>(value.clone()).and_then(|v| v.validate());
        assert_eq!(
            result.is_ok(),
            case["accepted"],
            "{}: {result:?}",
            case["name"]
        );
        assert_eq!(call("manifest.validate", &value).is_ok(), case["accepted"]);
        if !case["accepted"].as_bool().unwrap() {
            continue;
        }
        if let Some(key) = case.get("empty_chunk") {
            assert!(value["chunks"].get(key.as_str().unwrap()).is_none());
        }
        if case.get("preserves_entity") == Some(&Value::Bool(true)) {
            assert_eq!(
                value["entities"][0]["id"],
                fixture["catalog"]["entities"][0]["id"]
            );
            assert_eq!(
                value["entities"][0]["asset"],
                fixture["catalog"]["entities"][0]["asset"]
            );
            assert_ne!(
                value["entities"][0]["owner_chunk"],
                fixture["catalog"]["entities"][0]["owner_chunk"]
            );
        }
        if case.get("shared_asset") == Some(&Value::Bool(true)) {
            assert_eq!(value["entities"][0]["asset"], value["entities"][1]["asset"]);
            assert_ne!(value["entities"][0]["id"], value["entities"][1]["id"]);
        }
    }
}

#[test]
fn selectors_remain_compact_and_canonical() {
    let profile = Profile::default();
    let forward = profile.range(&["-1,-1,-1..1,1,1".into()]).unwrap();
    assert_eq!(forward.len(), 27);
    assert_eq!(
        forward,
        profile
            .range(&["1,1,1..-1,-1,-1".into(), "0,0,0".into()])
            .unwrap()
    );
    for selector in [
        "+1,0,0",
        "01,0,0",
        "-0,0,0",
        " 1,0,0",
        "\u{ff11},0,0",
        "4,0,0",
        "0,0",
        "0,0,0..0,0,0..0,0,0",
    ] {
        assert!(profile.range(&[selector.into()]).is_err(), "{selector}");
    }
    let mut large = profile;
    large.min_chunk = [0; 3];
    large.max_chunk = [32; 3];
    large.validate().unwrap();
    assert_eq!(
        large.range(&["31,31,31..0,0,0".into()]).unwrap().len(),
        32768
    );
    assert!(
        large
            .intersections(&Bounds {
                min: [0; 3],
                max: [1024000; 3]
            })
            .is_err()
    );
}
