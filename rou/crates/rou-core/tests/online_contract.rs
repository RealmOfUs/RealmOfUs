use rou_core::{online, ops};
use serde_json::{Value, json};

#[test]
fn repository_addresses_are_canonical_and_safe() {
    let source = online::parse_source("rou://localhost:3000/realm/garden@v1").unwrap();
    assert_eq!(
        serde_json::to_value(source).unwrap(),
        json!({
            "kind": "repository",
            "address": "rou://localhost:3000/realm/garden@v1",
            "host": "localhost:3000",
            "owner": "realm",
            "repository": "garden",
            "tag": "v1",
            "origin": "http://localhost:3000"
        })
    );
    let latest = online::parse_source("rou://code.example/realm/garden").unwrap();
    assert_eq!(serde_json::to_value(latest).unwrap()["tag"], Value::Null);
    assert_eq!(
        serde_json::to_value(online::parse_source("rou://code.example/a/b@release/one").unwrap())
            .unwrap()["origin"],
        "https://code.example"
    );
}

#[test]
fn repository_addresses_reject_ambiguous_or_credentialed_inputs() {
    let commit = "0123456789abcdef0123456789abcdef01234567";
    for address in [
        "rou://user:pass@example.test/a/b@v1",
        "rou://example.test/a/b@v1#fragment",
        "rou://example.test/a/b@v1?download=1",
        "rou://example.test/a/b.git@v1",
        "rou://example.test/a/b@main..next",
        "rou://example.test/a/b@bad tag",
        "rou://example.test/a/b@bad@tag",
        "rou://example.test/a/b@bad%20tag",
        "rou://example.test/a/b/c@v1",
        "rou://example.test/a%2fb/c@v1",
        "rou://example.test/a/b@",
        &format!("rou://example.test/a/b@{commit}"),
    ] {
        let error = online::parse_source(address).unwrap_err();
        assert_eq!(error.code, "invalid_url", "{address}");
    }
}

#[test]
fn static_sources_name_an_uncredentialed_manifest() {
    let source = online::parse_source("http://localhost:8000/view/realm.json").unwrap();
    assert_eq!(
        serde_json::to_value(source).unwrap(),
        json!({
            "kind": "static",
            "address": "http://localhost:8000/view/realm.json",
            "manifest_url": "http://localhost:8000/view/realm.json",
            "base_url": "http://localhost:8000/view"
        })
    );
    for address in [
        "ftp://example.test/view/realm.json",
        "https://user@example.test/view/realm.json",
        "https://example.test/view/version.json",
        "https://example.test/view/../realm.json",
        "https://example.test/view/realm.json?raw=1",
    ] {
        assert!(online::parse_source(address).is_err(), "{address}");
    }
}

#[test]
fn release_annotations_round_trip_and_reject_ambiguity() {
    let digest = "a".repeat(64);
    let line = online::release_annotation(&digest).unwrap();
    assert_eq!(line, format!("view-root: {digest}"));
    assert_eq!(
        online::parse_release_annotation(&format!("Realm release v1\n\n{line}\n")).unwrap(),
        Some(digest.clone())
    );
    assert_eq!(
        online::parse_release_annotation("ordinary tag").unwrap(),
        None
    );
    assert!(online::release_annotation(&"A".repeat(64)).is_err());
    assert!(online::parse_release_annotation("view-root: short").is_err());
    assert!(online::parse_release_annotation(&format!("{line}\n{line}")).is_err());
    for tag in [
        "bad@tag",
        "bad%tag",
        "bad#tag",
        "refs/heads/main",
        "refs/tags/v1",
    ] {
        assert!(online::validate_release_tag(tag).is_err(), "{tag}");
    }
}

#[test]
fn online_rules_are_available_through_the_shared_operation_table() {
    let parsed: Value = serde_json::from_slice(
        &ops::dispatch("url.parse", br#""rou://localhost:3000/realm/garden@v1""#).unwrap(),
    )
    .unwrap();
    assert_eq!(parsed["tag"], "v1");
    let digest = "b".repeat(64);
    let request = serde_json::to_vec(&json!({
        "action": "create",
        "view_root_sha256": digest
    }))
    .unwrap();
    let annotation: Value =
        serde_json::from_slice(&ops::dispatch("release.annotation", &request).unwrap()).unwrap();
    assert_eq!(
        annotation["annotation"],
        format!("view-root: {}", "b".repeat(64))
    );
    let parsed: Value = serde_json::from_slice(
        &ops::dispatch(
            "release.annotation",
            br#"{"action":"parse","annotation":"ordinary release"}"#,
        )
        .unwrap(),
    )
    .unwrap();
    assert!(parsed["view_root_sha256"].is_null());
}
