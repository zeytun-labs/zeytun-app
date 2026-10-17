use serde_json::json;
use zeytun_lib::core::{
    compiler::{
        profile_converter::ProfileConverter, runtime_profile_compiler::RuntimeProfileCompiler,
    },
    dto::*,
    subscription::{SubHeaders, SubscriptionManager},
    zeytun_core_config::ZeytunCoreConfigBuilder,
};

fn node(tag: &str, source: Option<&str>) -> Proxy {
    let mut p = zeytun_lib::core::link_parser::proxy_from_link(
        "socks5://user:secret@example.test:1080#Node",
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
fn source(id: &str) -> SubscriptionSource {
    serde_json::from_value(json!({"id":id,"generation":0,"position":0,"settings":{"name":id,"url":"https://example.test/private","enabled":true,"auto_update":true,"update_interval_hours":24}})).unwrap()
}
fn fixture() -> ZeytunCore {
    let mut p = ZeytunCore {
        proxies: vec![
            node("a", Some("a")),
            node("b", Some("b")),
            node("manual", None),
        ],
        ..ZeytunCore::default()
    };
    let mut policy = p.policies[0].clone();
    policy.tag = "authored".into();
    policy.name = "Authored".into();
    policy.members = Some(vec!["a".into(), "b".into(), "manual".into()]);
    policy.selected_member_tag = Some("a".into());
    policy.weights = Some(vec![2, 5, 9]);
    let mut shared = policy.clone();
    shared.tag = "shared".into();
    p.policies.extend([policy, shared]);
    p.normalize_policy_graph();
    p
}
fn config(p: &ZeytunCore) -> serde_json::Value {
    let runtime = RuntimeProfileCompiler::new()
        .compile(&ProfileConverter::convert(p))
        .unwrap();
    ZeytunCoreConfigBuilder::new(false, false)
        .build(&runtime, &p.outbound_mode)
        .unwrap()
}
#[test]
fn policies_and_runtime_are_provenance_independent() {
    let subscribed = fixture();
    let mut manual = subscribed.clone();
    for p in &mut manual.proxies {
        p.origin = ProxyOrigin::Manual;
        p.source_id = None;
    }
    assert_eq!(config(&manual), config(&subscribed));
    assert_eq!(
        serde_json::to_value(&manual.policies).unwrap(),
        serde_json::to_value(&subscribed.policies).unwrap()
    );
}
#[test]
fn refresh_only_expands_root_registry_and_pause_keeps_authored_intent() {
    let mut p = fixture();
    let before = serde_json::to_value(&p.policies[1..]).unwrap();
    let mut s = source("a");
    SubscriptionManager::reconcile(
        &mut p,
        &s,
        SubscriptionManager::stage(
            "socks5://user:secret@example.test:1080#Renamed\nsocks5://new.test:1080#New",
            SubHeaders::default(),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(serde_json::to_value(&p.policies[1..]).unwrap(), before);
    let new = p.proxies.iter().find(|n| n.title == "New").unwrap();
    assert!(p.policies[0].members.as_ref().unwrap().contains(&new.tag));
    assert!(!p.policies[1].members.as_ref().unwrap().contains(&new.tag));
    let runtime = config(&p);
    s.settings.enabled = false;
    s.settings.name = "Renamed source".into();
    assert!(SubscriptionManager::reconcile(
        &mut p,
        &s,
        SubscriptionManager::stage("socks5://other.test:1080", SubHeaders::default()).unwrap()
    )
    .is_err());
    assert_eq!(config(&p), runtime);
    assert_eq!(serde_json::to_value(&p.policies[1..]).unwrap(), before);
}

#[test]
fn unavailable_nodes_block_without_stale_credentials_and_filter_weight_pairs() {
    let mut p = fixture();
    p.proxies[0].available = false;
    p.policies[1].kind = Some(ProxyPolicyType::Balancer);
    let cfg = config(&p);
    let out = cfg["outbounds"].as_array().unwrap();
    assert_eq!(
        out.iter().find(|o| o["tag"] == "a").unwrap()["type"],
        "block"
    );
    assert!(
        !serde_json::to_string(out.iter().find(|o| o["tag"] == "a").unwrap())
            .unwrap()
            .contains("secret")
    );
    let policy = out.iter().find(|o| o["tag"] == "authored").unwrap();
    assert_eq!(policy["outbounds"], json!(["b", "manual"]));
    assert_eq!(policy["weights"], json!([5, 9]));
    let selected = out.iter().find(|o| o["tag"] == "shared").unwrap();
    assert_eq!(selected["default"], "b");
    assert_eq!(p.policies[2].selected_member_tag.as_deref(), Some("a"));
}

#[test]
fn empty_policies_chain_final_dns_and_live_rules_fail_closed() {
    let mut p = fixture();
    for n in &mut p.proxies {
        n.available = false;
    }
    p.policies[1].kind = Some(ProxyPolicyType::Urltest);
    p.policies[2].kind = Some(ProxyPolicyType::Balancer);
    let mut chain = node("chain", None);
    chain.protocol = "chain".into();
    chain.config = Some(json!({"tag":"chain","name":"Chain","type":"chain","proxies":["a","b"]}));
    p.proxies.push(chain);
    p.rules[0].outbound = "a".into();
    p.rules.push(serde_json::from_value(json!({"id":2,"kind":"DOMAIN","value":"example.test","outbound":"a","enabled":true,"comment":""})).unwrap());
    p.temp_rules.push(serde_json::from_value(json!({"id":3,"kind":"DOMAIN","value":"temp.test","outbound":"a","enabled":true,"comment":"","expires_at":i64::MAX})).unwrap());
    p.dns = Some(DnsConfig {
        mode: None,
        servers: Some(vec![DnsServer {
            tag: "dns".into(),
            name: "DNS".into(),
            address: "1.1.1.1".into(),
            detour: Some("a".into()),
        }]),
        ..Default::default()
    });
    let cfg = config(&p);
    let out = cfg["outbounds"].as_array().unwrap();
    for tag in [
        "a",
        "b",
        "manual",
        "chain",
        "authored",
        "shared",
        "root-policy",
    ] {
        assert_eq!(
            out.iter().find(|o| o["tag"] == tag).unwrap()["type"],
            "block",
            "{tag}"
        );
    }
    assert_eq!(cfg["route"]["final"], "a");
    assert_eq!(
        cfg["route"]["live_rules"]["permanent"][0]["rule"]["outbound"],
        "block"
    );
    assert_eq!(
        cfg["route"]["live_rules"]["temp"][0]["rule"]["outbound"],
        "block"
    );
    assert_eq!(cfg["dns"]["servers"][0]["detour"], "block");
}

#[test]
fn raw_graph_unknown_targets_and_cycles_never_bypass_live_validation() {
    let mut p = fixture();
    p.rules.push(serde_json::from_value(json!({"id":2,"kind":"DOMAIN","value":"example.test","outbound":"unknown","enabled":true,"comment":""})).unwrap());
    assert!(RuntimeProfileCompiler::new()
        .compile(&ProfileConverter::convert(&p))
        .is_err());
    p.rules.pop();
    p.policies[1].members = Some(vec!["authored".into()]);
    p.policies[1].selected_member_tag = Some("authored".into());
    p.policies[1].weights = None;
    assert!(RuntimeProfileCompiler::new()
        .compile(&ProfileConverter::convert(&p))
        .is_err());
}

#[test]
fn malformed_proxy_final_never_falls_back_to_direct() {
    let mut p = fixture();
    p.proxies[0].config = Some(json!({"type":"invalid"}));
    p.proxies[0].link = "invalid".into();
    p.rules[0].outbound = "a".into();
    let cfg = config(&p);
    assert_eq!(cfg["route"]["final"], "a");
    assert_eq!(
        cfg["outbounds"]
            .as_array()
            .unwrap()
            .iter()
            .find(|o| o["tag"] == "a")
            .unwrap()["type"],
        "block"
    );
}

#[test]
fn compiled_live_overlay_validates_sessions_and_matches_static_resolved_targets() {
    let mut p = fixture();
    p.proxies[0].available = false;
    p.session_rules.push(serde_json::from_value(json!({"id":8,"kind":"DOMAIN","value":"session.test","outbound":"a","enabled":true,"comment":"","expires_at":0,"session":true})).unwrap());
    let body = RuntimeProfileCompiler::live_rules_payload(&p, true).unwrap();
    // A hot overlay may target a core still holding the old credentials for
    // tag a. Block must be canonical, not merely a future config placeholder.
    assert_eq!(body["temp"][0]["rule"]["outbound"], "block");
    p.session_rules[0].outbound = "unknown".into();
    assert!(RuntimeProfileCompiler::live_rules_payload(&p, true).is_err());
}

#[test]
fn chain_with_unavailable_hop_and_proxy_detour_cannot_skip_security_hop() {
    let mut p = fixture();
    p.proxies[0].available = false;
    let mut chain = node("chain", None);
    chain.protocol = "chain".into();
    chain.config =
        Some(json!({"type":"chain","tag":"chain","name":"Chain","proxies":["b","a","manual"]}));
    p.proxies.push(chain);
    p.proxies[2].config.as_mut().unwrap()["detour"] = json!("a");
    let cfg = config(&p);
    for tag in ["chain", "manual"] {
        assert_eq!(
            cfg["outbounds"]
                .as_array()
                .unwrap()
                .iter()
                .find(|o| o["tag"] == tag)
                .unwrap()["type"],
            "block"
        );
    }
}

#[test]
fn factory_invalid_node_blocks_dependency_closure_and_warns_without_credentials() {
    use zeytun_lib::core::compiler::warning::CompileWarning;
    let mut p = fixture();
    p.proxies[0].config.as_mut().unwrap()["version"] = json!(0);
    p.proxies[2].config.as_mut().unwrap()["detour"] = json!("a");
    p.rules[0].outbound = "a".into();
    let runtime = RuntimeProfileCompiler::new()
        .compile(&ProfileConverter::convert(&p))
        .unwrap();
    assert!(runtime.warnings.iter().any(|w|matches!(w,CompileWarning::ProxyServerIgnored{proxy_server_tag,..} if proxy_server_tag=="a")));
    assert!(!format!("{:?}", runtime.warnings).contains("secret"));
    let cfg = config(&p);
    for tag in ["a", "manual"] {
        assert_eq!(
            cfg["outbounds"]
                .as_array()
                .unwrap()
                .iter()
                .find(|o| o["tag"] == tag)
                .unwrap()["type"],
            "block"
        );
    }
    assert_eq!(cfg["route"]["final"], "a");
}

#[test]
fn explicitly_authored_direct_member_is_preserved_not_invented_as_fallback() {
    let mut p = fixture();
    p.policies[1].members = Some(vec!["direct".into(), "a".into()]);
    p.policies[1].weights = Some(vec![3, 7]);
    p.policies[1].selected_member_tag = Some("direct".into());
    p.normalize_policy_graph();
    assert_eq!(
        p.policies[1].members.as_ref().unwrap(),
        &vec!["direct".to_string(), "a".to_string()]
    );
    assert_eq!(p.policies[1].selected_member_tag.as_deref(), Some("direct"));
    let cfg = config(&p);
    let policy = cfg["outbounds"]
        .as_array()
        .unwrap()
        .iter()
        .find(|o| o["tag"] == "authored")
        .unwrap();
    assert_eq!(policy["default"], "direct");
}
