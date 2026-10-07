use serde_json::json;
use zeytun_lib::core::{
    dto::*,
    profile_document::{ExportMode, ProfileDocument, CURRENT_SCHEMA_VERSION},
};

fn sample_proxy(tag: &str, source_id: Option<&str>) -> Proxy {
    let mut p = zeytun_lib::core::link_parser::proxy_from_link(
        "socks5://user:secret@127.0.0.1:1080#Node",
        ProxyOrigin::Manual,
        tag.into(),
    )
    .unwrap();
    if let Some(src) = source_id {
        p.origin = ProxyOrigin::Subscription;
        p.source_id = Some(src.into());
    }
    p
}

fn sample_source(id: &str) -> SubscriptionSource {
    serde_json::from_value(json!({
        "id": id,
        "generation": 0,
        "position": 0,
        "settings": {
            "name": id,
            "url": "https://example.test/sub",
            "enabled": true,
            "auto_update": true,
            "update_interval_hours": 24
        }
    }))
    .unwrap()
}

fn fixture_profile() -> (ZeytunCore, Vec<SubscriptionSource>) {
    let sources = vec![sample_source("src1"), sample_source("src2")];
    let mut profile = ZeytunCore {
        id: "prof-1".into(),
        proxies: vec![
            sample_proxy("node-manual", None),
            sample_proxy("node-sub1", Some("src1")),
            sample_proxy("node-sub2", Some("src2")),
        ],
        ..ZeytunCore::default()
    };

    let policy = ProxyPolicy {
        tag: "policy-main".into(),
        name: "Main Policy".into(),
        kind: Some(ProxyPolicyType::Selector),
        members: Some(vec!["node-manual".into(), "node-sub1".into()]),
        selected_member_tag: Some("node-sub1".into()),
        test_url: None,
        interval_seconds: None,
        tolerance_ms: None,
        strategy: None,
        weights: None,
    };
    profile.policies.push(policy);
    profile.normalize_policy_graph();

    (profile, sources)
}

#[test]
fn full_export_roundtrip_equivalence() {
    let (profile, sources) = fixture_profile();
    let encoded = ProfileDocument::encode(
        "Test Profile",
        None,
        &profile,
        &sources,
        ExportMode::Full,
        vec![],
    )
    .unwrap();

    let decoded = ProfileDocument::decode(&encoded).unwrap();
    assert_eq!(decoded.format, "zeytun-profile");
    assert_eq!(decoded.schema_version, CURRENT_SCHEMA_VERSION);
    assert_eq!(decoded.profile.name, "Test Profile");
    assert_eq!(decoded.profile.sources.len(), 2);
    assert_eq!(decoded.profile.proxies.len(), 3);

    let (imported, imported_sources) = decoded.to_zeytun_core("imported-id");
    assert_eq!(imported.id, "imported-id");
    assert_eq!(imported.proxies.len(), profile.proxies.len());
    assert_eq!(imported_sources.len(), sources.len());

    let preview = decoded.preview();
    assert_eq!(preview.mode, ExportMode::Full);
    assert_eq!(preview.source_count, 2);
    assert_eq!(preview.proxy_count, 3);
}

#[test]
fn snapshot_export_converts_nodes_to_manual_and_strips_sources() {
    let (profile, sources) = fixture_profile();
    let encoded = ProfileDocument::encode(
        "Test Profile",
        None,
        &profile,
        &sources,
        ExportMode::Snapshot,
        vec![],
    )
    .unwrap();

    let decoded = ProfileDocument::decode(&encoded).unwrap();
    assert_eq!(decoded.profile.sources.len(), 0);

    for p in &decoded.profile.proxies {
        assert_eq!(p.origin, ProxyOrigin::Manual);
        assert!(p.source_id.is_none());
    }

    let preview = decoded.preview();
    assert_eq!(preview.mode, ExportMode::Snapshot);
    assert_eq!(preview.source_count, 0);
}

#[test]
fn rejects_unsupported_version_and_format() {
    let raw = json!({
        "format": "wrong-format",
        "schema_version": 1,
        "profile": {
            "name": "Test",
            "sources": [],
            "proxies": [],
            "policies": [],
            "rules": []
        },
        "assets": []
    })
    .to_string();

    assert!(ProfileDocument::decode(&raw).is_err());

    let raw_future_ver = json!({
        "format": "zeytun-profile",
        "schema_version": 99,
        "profile": {
            "name": "Test",
            "sources": [],
            "proxies": [],
            "policies": [],
            "rules": []
        },
        "assets": []
    })
    .to_string();

    assert!(ProfileDocument::decode(&raw_future_ver).is_err());
}

#[test]
fn rejects_duplicate_tags_and_reserved_names() {
    let (mut profile, sources) = fixture_profile();
    // Collide with reserved tag
    profile.proxies[0].tag = "direct".into();

    assert!(
        ProfileDocument::encode(
            "Test Profile",
            None,
            &profile,
            &sources,
            ExportMode::Full,
            vec![]
        )
        .is_err()
            || ProfileDocument::decode(
                &ProfileDocument::encode(
                    "Test Profile",
                    None,
                    &profile,
                    &sources,
                    ExportMode::Full,
                    vec![]
                )
                .unwrap()
            )
            .is_err()
    );
}
