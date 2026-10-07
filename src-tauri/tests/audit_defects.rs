//! Regression tests for Tasks 6-8 audit defects A, B, and C.

use serde_json::json;
use zeytun_lib::core::{
    compiler::{
        profile_converter::ProfileConverter, runtime_profile_compiler::RuntimeProfileCompiler,
    },
    dto::*,
    subscription::{SubHeaders, SubscriptionManager},
    zeytun_core_config::ZeytunCoreConfigBuilder,
};

fn node_with_host(tag: &str, host: &str, source: Option<&str>) -> Proxy {
    let mut p = zeytun_lib::core::link_parser::proxy_from_link(
        &format!("socks5://user:secret@{host}:1080#{tag}"),
        ProxyOrigin::Manual,
        tag.into(),
    )
    .unwrap();
    if let Some(source) = source {
        p.origin = ProxyOrigin::Subscription;
        p.source_id = Some(source.into());
    }
    p
}

fn node(tag: &str, source: Option<&str>) -> Proxy {
    node_with_host(tag, "example.test", source)
}

fn source(id: &str) -> SubscriptionSource {
    serde_json::from_value(json!({
        "id": id,
        "generation": 0,
        "position": 0,
        "settings": {
            "name": id,
            "url": "https://example.test/private",
            "enabled": true,
            "auto_update": true,
            "update_interval_hours": 24
        }
    }))
    .unwrap()
}

/// Defect A: hot rules targeting a policy should resolve through the compiled
/// policy, which drops unavailable members. When the selected node becomes
/// unavailable, the compiled runtime policy must fallback to a remaining member.
#[test]
fn defect_a_policy_fallback_on_unavailable_selected_member() {
    let mut p = ZeytunCore {
        proxies: vec![node("a", Some("src")), node("b", Some("src"))],
        ..ZeytunCore::default()
    };
    let mut policy = p.policies[0].clone();
    policy.tag = "P".into();
    policy.name = "Policy P".into();
    policy.members = Some(vec!["a".into(), "b".into()]);
    policy.selected_member_tag = Some("a".into());
    p.policies.push(policy);
    p.normalize_policy_graph();

    // Mark 'a' as unavailable.
    p.proxies[0].available = false;

    let runtime = RuntimeProfileCompiler::new()
        .compile(&ProfileConverter::convert(&p))
        .unwrap();

    let cfg = ZeytunCoreConfigBuilder::new(false, false)
        .build(&runtime, &p.outbound_mode)
        .unwrap();

    let outbounds = cfg["outbounds"].as_array().unwrap();

    // 'a' compiles to block
    assert_eq!(
        outbounds.iter().find(|o| o["tag"] == "a").unwrap()["type"],
        "block"
    );

    // Policy P drops unavailable 'a' and falls back to 'b'
    let p_out = outbounds.iter().find(|o| o["tag"] == "P").unwrap();
    assert_eq!(p_out["outbounds"], json!(["b"]));
    assert_eq!(p_out["default"], "b");

    // The authored intent in DTO is preserved
    assert_eq!(p.policies[1].selected_member_tag.as_deref(), Some("a"));
}

/// Defect B: chain-hop tag collision with authored proxy identities.
/// A chain C=[a,b,c] generates intermediate hops. If an authored proxy
/// also has tag "C-b", all emitted outbound tags must be strictly unique.
#[test]
fn defect_b_chain_hop_does_not_collide_with_authored_proxy() {
    let mut p = ZeytunCore {
        proxies: vec![
            node("a", None),
            node("b", None),
            node("c", None),
            node("C-b", None), // authored proxy whose tag matches a chain hop
        ],
        ..ZeytunCore::default()
    };

    let mut chain = node("C", None);
    chain.protocol = "chain".into();
    chain.config = Some(json!({
        "tag": "C",
        "name": "Chain C",
        "type": "chain",
        "proxies": ["a", "b", "c"]
    }));
    p.proxies.push(chain);

    let runtime = RuntimeProfileCompiler::new()
        .compile(&ProfileConverter::convert(&p))
        .unwrap();

    let cfg = ZeytunCoreConfigBuilder::new(false, false)
        .build(&runtime, &p.outbound_mode)
        .unwrap();

    let outbounds = cfg["outbounds"].as_array().unwrap();

    // Every emitted outbound tag MUST be strictly unique
    let mut seen = std::collections::HashSet::new();
    for o in outbounds {
        let tag = o["tag"].as_str().unwrap();
        assert!(
            seen.insert(tag),
            "duplicate outbound tag `{tag}`: chain intermediate collided with authored proxy"
        );
    }

    // Both the authored C-b and the chain C must exist
    assert!(outbounds.iter().any(|o| o["tag"] == "C-b"));
    assert!(outbounds.iter().any(|o| o["tag"] == "C"));
}

/// Defect C: effective selection changes must be reported in SyncSummary.
/// When refresh removes a pinned member while another survives, authored
/// selection remains unchanged and SyncSummary reports the effective fallback.
#[test]
fn defect_c_sync_summary_reports_effective_selection_fallback() {
    let mut p = ZeytunCore {
        proxies: vec![
            node_with_host("a", "a.test", Some("src")),
            node_with_host("b", "b.test", Some("src")),
        ],
        ..ZeytunCore::default()
    };
    let mut policy = p.policies[0].clone();
    policy.tag = "P".into();
    policy.name = "Policy P".into();
    policy.members = Some(vec!["a".into(), "b".into()]);
    policy.selected_member_tag = Some("a".into());
    p.policies.push(policy);
    p.normalize_policy_graph();

    let s = source("src");

    // Refresh response only contains 'b' (node 'a' removed by upstream)
    let staged =
        SubscriptionManager::stage("socks5://user:secret@b.test:1080#b", SubHeaders::default())
            .unwrap();

    let sync_res = SubscriptionManager::reconcile(&mut p, &s, staged).unwrap();

    // Authored selection on the policy is preserved
    assert_eq!(p.policies[1].selected_member_tag.as_deref(), Some("a"));

    // Sync summary reports the effective fallback
    assert!(
        !sync_res.summary.effective_fallbacks.is_empty(),
        "summary must report effective selection fallback when pinned member is removed"
    );
    assert!(sync_res
        .summary
        .effective_fallbacks
        .iter()
        .any(|fb| fb.contains("Policy P") && fb.contains("b")));
}

/// Reality TLS inbounds frequently rotate shortId (`reality_sid`) and camouflage
/// domain (`sni`) across downloads. Reconcile must recognize the node as kept,
/// not +1 / -1 churn.
#[test]
fn defect_d_reality_ephemeral_fields_rotation_preserves_node_identity() {
    let s = source("src");
    let link_v1 = "vless://24c8bbb2-9906-4322-905c-8944ec67321f@31.56.117.67:50966?encryption=none&fp=chrome&pbk=j4G7f_zg6Aqdv1W0FKoZXLvMq89GFiY50bj07KYhTxU&security=reality&sid=1233192ecbd61539&sni=www.cloudflare.com&spx=%2F8971bc40bb4c251&type=tcp#Riga%20Reality";
    let staged_v1 = SubscriptionManager::stage(link_v1, SubHeaders::default()).unwrap();

    let mut p = ZeytunCore::default();
    let res1 = SubscriptionManager::reconcile(&mut p, &s, staged_v1).unwrap();
    assert_eq!(res1.summary.proxies_added, 1);
    assert_eq!(res1.summary.proxies_kept, 0);
    assert_eq!(res1.summary.proxies_removed, 0);
    let original_tag = p.proxies[0].tag.clone();

    // Upstream server rotated sid to "90d80422" and sni to "timeline.www.cloudflare.com"
    let link_v2 = "vless://24c8bbb2-9906-4322-905c-8944ec67321f@31.56.117.67:50966?encryption=none&fp=chrome&pbk=j4G7f_zg6Aqdv1W0FKoZXLvMq89GFiY50bj07KYhTxU&security=reality&sid=90d80422&sni=timeline.www.cloudflare.com&spx=%2F8971bc40bb4c251&type=tcp#Riga%20Reality";
    let staged_v2 = SubscriptionManager::stage(link_v2, SubHeaders::default()).unwrap();

    let res2 = SubscriptionManager::reconcile(&mut p, &s, staged_v2).unwrap();
    assert_eq!(
        res2.summary.proxies_added, 0,
        "must not treat rotated reality sid/sni as a new proxy"
    );
    assert_eq!(res2.summary.proxies_kept, 1, "must keep existing proxy");
    assert_eq!(
        res2.summary.proxies_removed, 0,
        "must not remove existing proxy"
    );
    assert_eq!(
        p.proxies[0].tag, original_tag,
        "proxy tag must be preserved"
    );
    // Ensure the updated parameters landed in the kept node
    assert!(p.proxies[0].link.contains("sid=90d80422"));
}
