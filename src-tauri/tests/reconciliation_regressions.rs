use serde_json::json;
use zeytun_lib::core::{
    compiler::{
        profile_converter::ProfileConverter, runtime_profile_compiler::RuntimeProfileCompiler,
    },
    dto::*,
    link_parser,
    profile_document::{ExportMode, ProfileDocument},
    subscription::{SubHeaders, SubscriptionManager},
    zeytun_core_config::ZeytunCoreConfigBuilder,
};

fn source() -> SubscriptionSource {
    serde_json::from_value(json!({"id":"a","generation":0,"position":0,"settings":{"name":"A","url":"https://example.test/sub","enabled":true,"auto_update":true,"update_interval_hours":24}})).unwrap()
}

fn node(tag: &str, link: &str, subscribed: bool) -> Proxy {
    let mut p = link_parser::proxy_from_link(link, ProxyOrigin::Manual, tag.into()).unwrap();
    if subscribed {
        p.origin = ProxyOrigin::Subscription;
        p.source_id = Some("a".into());
    }
    p
}

#[test]
fn whitespace_proxy_tag_should_not_leave_dangling_runtime_detour() {
    let dependency = node(" saved ", "socks5://saved.test:1080#Saved", true);
    let mut downstream = node("manual", "socks5://manual.test:1080#Manual", false);
    downstream.config.as_mut().unwrap()["detour"] = json!(" saved ");
    let mut p = ZeytunCore {
        proxies: vec![downstream, dependency],
        ..ZeytunCore::default()
    };
    p.normalize_policy_graph();
    let doc =
        ProfileDocument::encode("Probe", None, &p, &[source()], ExportMode::Full, vec![]).unwrap();
    ProfileDocument::decode(&doc).unwrap();
    let staged =
        SubscriptionManager::stage("socks5://saved.test:1080#Renamed", SubHeaders::default())
            .unwrap();
    SubscriptionManager::reconcile(&mut p, &source(), staged).unwrap();
    let runtime = RuntimeProfileCompiler::new()
        .compile(&ProfileConverter::convert(&p))
        .unwrap();
    let cfg = ZeytunCoreConfigBuilder::new(false, false)
        .build(&runtime, &p.outbound_mode)
        .unwrap();
    let outbounds = cfg["outbounds"].as_array().unwrap();
    let detour = outbounds.iter().find(|o| o["tag"] == "manual").unwrap()["detour"]
        .as_str()
        .unwrap();
    assert!(
        outbounds.iter().any(|o| o["tag"] == detour),
        "runtime detour {detour:?} has no outbound; emitted tags: {:?}",
        outbounds
            .iter()
            .map(|o| o["tag"].clone())
            .collect::<Vec<_>>()
    );
}

#[test]
fn link_only_download_detour_should_retain_removed_dependency() {
    let saved = node("saved", "socks5://saved.test:1080#Saved", true);
    let mut downstream = node(
        "manual",
        "vless://11111111-1111-4111-8111-111111111111@manual.test:443?type=xhttp&path=%2F#Manual",
        false,
    );
    downstream.config.as_mut().unwrap()["transport"]["download"] = json!({"detour":"saved"});
    downstream.link = link_parser::proxy_to_link(&downstream).unwrap();
    downstream.config = None;
    let parsed = link_parser::proxy_to_proxy_server(&downstream).unwrap();
    let parsed_json = serde_json::to_value(parsed).unwrap();
    assert_eq!(parsed_json["transport"]["download"]["detour"], "saved");
    let mut p = ZeytunCore {
        proxies: vec![downstream, saved],
        ..ZeytunCore::default()
    };
    p.normalize_policy_graph();
    assert_eq!(p.policies[0].selected_member_tag.as_deref(), Some("manual"));
    let staged =
        SubscriptionManager::stage("socks5://new.test:1080#New", SubHeaders::default()).unwrap();
    SubscriptionManager::reconcile(&mut p, &source(), staged).unwrap();
    let model = ProfileConverter::convert(&p);
    let runtime = RuntimeProfileCompiler::new().compile(&model).unwrap();
    let cfg = ZeytunCoreConfigBuilder::new(false, false)
        .build(&runtime, &p.outbound_mode)
        .unwrap();
    let manual = cfg["outbounds"]
        .as_array()
        .unwrap()
        .iter()
        .find(|o| o["tag"] == "manual")
        .unwrap();
    assert!(
        p.proxies
            .iter()
            .any(|proxy| proxy.tag == "saved" && !proxy.available),
        "link-only download detour lost its dependency; downstream runtime type: {}",
        manual["type"]
    );
    assert_eq!(manual["type"], "block");

    // When the endpoint returns, the original identity makes the dependent
    // proxy usable again without editing the authored share-link reference.
    let staged =
        SubscriptionManager::stage("socks5://saved.test:1080#Returned", SubHeaders::default())
            .unwrap();
    SubscriptionManager::reconcile(&mut p, &source(), staged).unwrap();
    assert!(p
        .proxies
        .iter()
        .any(|proxy| proxy.tag == "saved" && proxy.available));
    let runtime = RuntimeProfileCompiler::new()
        .compile(&ProfileConverter::convert(&p))
        .unwrap();
    let cfg = ZeytunCoreConfigBuilder::new(false, false)
        .build(&runtime, &p.outbound_mode)
        .unwrap();
    let manual = cfg["outbounds"]
        .as_array()
        .unwrap()
        .iter()
        .find(|o| o["tag"] == "manual")
        .unwrap();
    assert_eq!(manual["type"], "vless");
    assert_eq!(manual["transport"]["downloadSettings"]["detour"], "saved");
}

fn link_only_proxy(detour: &str) -> Proxy {
    let mut proxy = node(
        "manual",
        "vless://11111111-1111-4111-8111-111111111111@manual.test:443?type=xhttp&path=%2F#Manual",
        false,
    );
    proxy.config.as_mut().unwrap()["transport"]["download"] = json!({"detour":detour});
    proxy.link = link_parser::proxy_to_link(&proxy).unwrap();
    proxy.config = None;
    proxy
}

#[test]
fn link_only_dependencies_are_validated_before_candidate_mutation() {
    for target in ["missing", "manual"] {
        let mut p = ZeytunCore {
            proxies: vec![
                link_only_proxy(target),
                node("saved", "socks5://saved.test:1080#Saved", true),
            ],
            ..ZeytunCore::default()
        };
        p.normalize_policy_graph();
        let before = serde_json::to_value(&p).unwrap();
        let staged =
            SubscriptionManager::stage("socks5://saved.test:1080#Saved", SubHeaders::default())
                .unwrap();
        assert!(SubscriptionManager::reconcile(&mut p, &source(), staged).is_err());
        assert_eq!(serde_json::to_value(&p).unwrap(), before);
        assert!(RuntimeProfileCompiler::new()
            .compile(&ProfileConverter::convert(&p))
            .is_err());
    }
}

#[test]
fn delete_preview_finds_link_only_download_dependencies() {
    use zeytun_lib::core::storage::{Db, Storage};

    let dir = std::env::temp_dir().join(format!("link-dependency-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir(&dir).unwrap();
    let db = Db::new(dir.join("fixture.db"));
    db.save_profile(&ZeytunCore::default()).unwrap();
    db.create_subscription("default", &source()).unwrap();
    let mut p = ZeytunCore {
        proxies: vec![
            link_only_proxy("saved"),
            node("saved", "socks5://saved.test:1080#Saved", true),
        ],
        ..ZeytunCore::default()
    };
    p.normalize_policy_graph();
    db.save_profile(&p).unwrap();
    let before = serde_json::to_value(db.load_profile("default").unwrap()).unwrap();
    let preview = db.subscription_delete_preview("default", "a", &[]).unwrap();
    assert!(preview.dependencies.iter().any(|reference| {
        reference.owner == "manual"
            && reference.proxy_tag == "saved"
            && matches!(reference.kind, SubscriptionDependencyKind::ProxyDetour)
    }));
    assert_eq!(
        serde_json::to_value(db.load_profile("default").unwrap()).unwrap(),
        before
    );
    drop(db);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn allocated_tags_are_used_for_all_outbound_reference_fields() {
    let mut manual = node(
        "manual",
        "vless://11111111-1111-4111-8111-111111111111@manual.test:443?type=xhttp&path=%2F#Manual",
        false,
    );
    manual.config.as_mut().unwrap()["detour"] = json!("saved");
    manual.config.as_mut().unwrap()["transport"]["download"] = json!({"detour":" saved "});
    manual.config.as_mut().unwrap()["transport"]["headers"] =
        json!({"detour":" saved ","proxies":"opaque"});
    let mut chain = node(" chain ", "socks5://chain.test:1080", false);
    chain.protocol = "chain".into();
    chain.config =
        Some(json!({"tag":" chain ","name":"Chain","type":"chain","proxies":[" saved ","manual"]}));
    let mut p = ZeytunCore {
        proxies: vec![
            manual,
            node(" saved ", "socks5://saved.test:1080", false),
            node("saved", "socks5://other.test:1080", false),
            chain,
        ],
        ..ZeytunCore::default()
    };
    let mut first = p.policies[0].clone();
    first.tag = " policy ".into();
    first.members = Some(vec![" saved ".into(), "saved".into()]);
    first.selected_member_tag = Some("saved".into());
    let mut second = first.clone();
    second.tag = "policy".into();
    second.members = Some(vec![" chain ".into()]);
    second.selected_member_tag = Some(" chain ".into());
    p.policies.extend([first, second]);
    p.rules[0].outbound = " saved ".into();
    p.rules.push(serde_json::from_value(json!({"id":2,"kind":"DOMAIN","value":"example.test","outbound":" policy ","enabled":true,"comment":""})).unwrap());
    p.temp_rules.push(serde_json::from_value(json!({"id":3,"kind":"DOMAIN","value":"temp.test","outbound":"saved","enabled":true,"comment":"","expires_at":i64::MAX})).unwrap());
    p.session_rules.push(serde_json::from_value(json!({"id":4,"kind":"DOMAIN","value":"session.test","outbound":" policy ","enabled":true,"comment":"","expires_at":0,"session":true})).unwrap());
    p.dns = Some(DnsConfig {
        servers: Some(vec![DnsServer {
            tag: "dns".into(),
            name: "DNS".into(),
            address: "1.1.1.1".into(),
            detour: Some("saved".into()),
        }]),
        ..Default::default()
    });
    p.rule_sets = Some(vec![RuleSet {
        id: "rs".into(),
        tag: "rs".into(),
        kind: "remote".into(),
        source: "https://example.test/rules.srs".into(),
        action: "policy".into(),
        download_policy: Some(" saved ".into()),
        comment: None,
        enabled: true,
        name: None,
    }]);
    p.normalize_policy_graph();
    let runtime = RuntimeProfileCompiler::new()
        .compile(&ProfileConverter::convert(&p))
        .unwrap();
    let proxy_tag = |id: &str| {
        runtime
            .outbounds
            .iter()
            .find(|o| o.source_proxy_server_id == id)
            .unwrap()
            .tag
            .as_str()
    };
    let policy_tag = |id: &str| {
        runtime
            .policies
            .iter()
            .find(|o| o.source_proxy_policy_id == id)
            .unwrap()
            .tag
            .as_str()
    };
    assert_ne!(proxy_tag(" saved "), proxy_tag("saved"));
    assert_ne!(policy_tag(" policy "), policy_tag("policy"));
    let cfg = ZeytunCoreConfigBuilder::new(false, false)
        .build(&runtime, &p.outbound_mode)
        .unwrap();
    let manual = cfg["outbounds"]
        .as_array()
        .unwrap()
        .iter()
        .find(|o| o["tag"] == "manual")
        .unwrap();
    assert_eq!(manual["detour"], proxy_tag("saved"));
    assert_eq!(
        manual["transport"]["downloadSettings"]["detour"],
        proxy_tag(" saved ")
    );
    assert_eq!(manual["transport"]["headers"]["detour"], " saved ");
    let chain = cfg["outbounds"]
        .as_array()
        .unwrap()
        .iter()
        .find(|o| o["tag"] == proxy_tag(" chain "))
        .unwrap();
    assert_eq!(chain["detour"], proxy_tag(" saved "));
    assert_eq!(
        chain["transport"]["downloadSettings"]["detour"],
        proxy_tag(" saved ")
    );
    assert_eq!(cfg["route"]["final"], proxy_tag(" saved "));
    assert_eq!(runtime.live_permanent[0].outbound, policy_tag(" policy "));
    assert_eq!(runtime.live_temp[0].outbound, proxy_tag("saved"));
    assert_eq!(
        runtime.dns.as_ref().unwrap().servers.as_ref().unwrap()[0]
            .detour
            .as_deref(),
        Some(proxy_tag("saved"))
    );
    let rs = &runtime.rule_sets.as_ref().unwrap()[0];
    assert_eq!(rs.action, policy_tag("policy"));
    assert_eq!(rs.download_policy.as_deref(), Some(proxy_tag(" saved ")));
    let body = RuntimeProfileCompiler::live_rules_payload(&p, true).unwrap();
    assert_eq!(body["temp"][1]["rule"]["outbound"], policy_tag(" policy "));
}
