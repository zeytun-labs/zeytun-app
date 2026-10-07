use serde_json::json;
use zeytun_lib::core::{
    dto::*,
    subscription::{SubHeaders, SubscriptionManager},
};

const LINK: &str = "socks5://user:secret@example.test:1080#Node";
#[test]
fn permissive_parser_must_not_accept_malformed_raw_credentials_or_ports() {
    use base64::{engine::general_purpose::STANDARD, Engine};
    let vmess =
        |value: serde_json::Value| format!("vmess://{}", STANDARD.encode(value.to_string()));
    let cases = [
        "ss://aes-128-gcm:synthetic-password@bad.test:99999#Broken".into(),
        vmess(json!({"add":"bad.test","port":99999,"id":"5e3daa0d-9c9a-4f0f-9a1d-6ce35eeb3fc3"})),
        vmess(json!({"add":"bad.test","port":443})),
    ];
    for broken in cases {
        let mut p = profile();
        let before = serde_json::to_value(&p).unwrap();
        let result = SubscriptionManager::apply_update(
            &mut p,
            &format!("{LINK}\n{broken}"),
            SubHeaders::default(),
        );
        assert!(
            !result.summary.errors.is_empty(),
            "accepted malformed raw link: {broken}"
        );
        assert_eq!(serde_json::to_value(&p).unwrap(), before);
        assert!(zeytun_lib::core::link_parser::proxy_from_link(
            &broken,
            ProxyOrigin::Manual,
            "manual".into()
        )
        .is_err());
        assert!(!result
            .summary
            .errors
            .join(" ")
            .contains("synthetic-password"));
    }
}
fn profile() -> ZeytunCore {
    let mut p = ZeytunCore::default();
    let mut proxy = zeytun_lib::core::link_parser::proxy_from_link(
        LINK,
        ProxyOrigin::Subscription,
        "saved".into(),
    )
    .unwrap();
    proxy.source_id = Some("a".into());
    p.proxies.push(proxy);
    p.normalize_policy_graph();
    p
}

#[test]
fn malformed_subscription_rejects_entire_body_without_mutation() {
    let cases = vec![
        "".to_string(),
        "<html>login token=secret</html>".into(),
        "{\"outbounds\":[]}".into(),
        "proxies:\n - name: nope".into(),
        format!("{LINK}\nunknown://secret@host"),
        format!("{LINK}\nsocks5://bad:99999"),
        format!("{LINK}\npromo token=secret"),
        "a".repeat(10 * 1024 * 1024 + 1),
    ];
    for raw in cases {
        let mut p = profile();
        let before = serde_json::to_vec(&p).unwrap();
        let result = SubscriptionManager::apply_update(&mut p, &raw, SubHeaders::default());
        assert!(
            !result.summary.errors.is_empty(),
            "accepted malformed payload"
        );
        assert_eq!(serde_json::to_vec(&p).unwrap(), before);
        assert!(result.userinfo.is_none());
        assert!(!result.summary.errors.join(" ").contains("secret"));
    }
}

#[test]
fn staged_parser_accepts_plain_base64_and_comments_and_redacts_errors() {
    use base64::Engine;
    let raw = format!("# comment\n\n{LINK}\n");
    assert!(SubscriptionManager::stage(&raw, SubHeaders::default()).is_ok());
    let encoded = base64::engine::general_purpose::STANDARD.encode(raw);
    assert!(SubscriptionManager::stage(&encoded, SubHeaders::default()).is_ok());
    let err = SubscriptionManager::stage(
        &format!("{LINK}\nunknown://secret@host"),
        SubHeaders::default(),
    )
    .err()
    .unwrap();
    let serialized = serde_json::to_value(&err).unwrap();
    assert_eq!(serialized["invalid_count"], json!(1));
    assert!(!err.to_string().contains("secret"));
}

#[test]
fn fetch_boundary_rejects_non_http_and_redacts_invalid_proxy() {
    let error = SubscriptionManager::fetch_url("file:///private/secret", None)
        .err()
        .unwrap();
    assert!(!error.contains("secret"));
    let error = SubscriptionManager::fetch_url("https://example.test/secret", Some("bad://secret"))
        .err()
        .unwrap();
    assert!(!error.contains("secret"));
}

fn source(id: &str) -> SubscriptionSource {
    serde_json::from_value(json!({"id":id,"generation":0,"position":0,"settings":{"name":id,"url":"https://example.test/private","enabled":true,"auto_update":true,"update_interval_hours":24}})).unwrap()
}

fn websocket_proxy() -> Proxy {
    zeytun_lib::core::link_parser::proxy_from_link(
        "vless://5e3daa0d-9c9a-4f0f-9a1d-6ce35eeb3fc3@ws.test:443?type=ws#WS",
        ProxyOrigin::Manual,
        "ws".into(),
    )
    .unwrap()
}

#[test]
fn arbitrary_headers_are_opaque_to_refresh_compiler_and_delete_preview() {
    use zeytun_lib::core::{compiler::profile_converter::ProfileConverter, storage::Storage};
    let f = DeleteFixture::new();
    let mut p = profile();
    let mut ws = websocket_proxy();
    ws.config.as_mut().unwrap()["transport"]["headers"] =
        json!({"detour":"ordinary-header-value", "proxies":"saved"});
    p.proxies.push(ws);
    p.normalize_policy_graph();
    p.policies[0].selected_member_tag = Some("ws".into());
    f.db.save_profile(&p).unwrap();
    let stage = SubscriptionManager::stage(LINK, SubHeaders::default()).unwrap();
    f.db.apply_subscription("default", &source("a"), stage, false, &[])
        .expect("opaque headers must not reject refresh");
    let resolved = ProfileConverter::convert(&p);
    assert!(resolved.proxy_servers.iter().any(|s| s.tag == "ws"));
    let preview =
        f.db.subscription_delete_preview("default", "a", &[])
            .unwrap();
    assert!(!preview.dependencies.iter().any(|d| d.owner == "ws"));
}

#[test]
fn xhttp_download_detour_is_a_real_dependency_in_refresh_compiler_and_preview() {
    use zeytun_lib::core::{compiler::profile_converter::ProfileConverter, storage::Storage};
    let f = DeleteFixture::new();
    let mut p = profile();
    let mut ws = websocket_proxy();
    ws.config.as_mut().unwrap()["network"] = json!("xhttp");
    ws.config.as_mut().unwrap()["transport"]["download"] = json!({"detour":"saved"});
    p.proxies.push(ws);
    p.normalize_policy_graph();
    p.policies[0].selected_member_tag = Some("ws".into());
    f.db.save_profile(&p).unwrap();
    let preview =
        f.db.subscription_delete_preview("default", "a", &[])
            .unwrap();
    assert!(preview
        .dependencies
        .iter()
        .any(|d| d.owner == "ws" && d.proxy_tag == "saved"));
    let stage =
        SubscriptionManager::stage("socks5://new.test:1080", SubHeaders::default()).unwrap();
    let (after, _) =
        f.db.apply_subscription("default", &source("a"), stage, false, &[])
            .unwrap();
    assert!(
        !after
            .proxies
            .iter()
            .find(|n| n.tag == "saved")
            .unwrap()
            .available
    );
    assert!(ProfileConverter::convert(&after)
        .blocked_proxy_tags
        .contains(&"ws".into()));
}

#[test]
fn refresh_is_scoped_preserves_identity_preferences_and_tombstones_intent() {
    let mut p = profile();
    p.proxies[0].enabled = false;
    let mut b = p.proxies[0].clone();
    b.tag = "b".into();
    b.source_id = Some("b".into());
    b.enabled = true;
    let mut manual = b.clone();
    manual.tag = "manual".into();
    manual.origin = ProxyOrigin::Manual;
    manual.source_id = None;
    p.proxies.extend([b, manual]);
    let mut policy = p.policies[0].clone();
    policy.tag = "user".into();
    policy.members = Some(vec!["saved".into(), "b".into()]);
    policy.weights = Some(vec![3, 7]);
    policy.selected_member_tag = Some("saved".into());
    p.policies.push(policy);
    let outside = serde_json::to_value(&p.proxies[1..]).unwrap();
    let authored = serde_json::to_value(&p.policies[1]).unwrap();
    let renamed = LINK.replace("#Node", "#Renamed");
    let stage = SubscriptionManager::stage(
        &format!("{renamed}\n{renamed}\nsocks5://other.test:1080#Node"),
        SubHeaders::default(),
    )
    .unwrap();
    let result = SubscriptionManager::reconcile(&mut p, &source("a"), stage).unwrap();
    assert_eq!(result.summary.proxies_added, 1);
    assert_eq!(result.summary.proxies_kept, 1);
    let saved = p.proxies.iter().find(|n| n.tag == "saved").unwrap();
    assert_eq!(saved.title, "Renamed");
    assert!(!saved.enabled);
    assert!(saved.available);
    assert_eq!(saved.config.as_ref().unwrap()["tag"], "saved");
    assert_eq!(
        serde_json::to_value(
            p.proxies
                .iter()
                .filter(|p| p.source_id.as_deref() != Some("a"))
                .collect::<Vec<_>>()
        )
        .unwrap(),
        outside
    );
    assert_eq!(serde_json::to_value(&p.policies[1]).unwrap(), authored);
    let stage = SubscriptionManager::stage(
        "socks5://user:changed@example.test:1080#Renamed",
        SubHeaders::default(),
    )
    .unwrap();
    let result = SubscriptionManager::reconcile(&mut p, &source("a"), stage).unwrap();
    assert_eq!(result.summary.proxies_added, 1);
    let old = p.proxies.iter().find(|p| p.tag == "saved").unwrap();
    assert!(!old.available);
    assert!(!old.enabled);
    assert_eq!(serde_json::to_value(&p.policies[1]).unwrap(), authored);
    assert_eq!(
        p.proxies
            .iter()
            .filter(|p| p.source_id.as_deref() == Some("a"))
            .count(),
        2,
        "unreferenced old node pruned, pinned one tombstoned"
    );
    let stage = SubscriptionManager::stage(LINK, SubHeaders::default()).unwrap();
    SubscriptionManager::reconcile(&mut p, &source("a"), stage).unwrap();
    let old = p.proxies.iter().find(|p| p.tag == "saved").unwrap();
    assert!(old.available);
    assert!(!old.enabled);
}

#[test]
fn accepted_snapshot_and_metadata_commit_atomically_and_reopen() {
    use zeytun_lib::core::storage::{Db, Storage};
    let path = std::env::temp_dir().join(format!("reconciliation-{}.db", uuid::Uuid::new_v4()));
    let db = Db::new(path.clone());
    db.save_profile(&ZeytunCore::default()).unwrap();
    let a = source("a");
    db.create_subscription("default", &a).unwrap();
    db.create_subscription("default", &source("b")).unwrap();
    let p = profile();
    db.save_profile(&p).unwrap();
    let before =
        serde_json::to_value(Db::new(path.clone()).load_profile("default").unwrap()).unwrap();
    let conn = rusqlite::Connection::open(&path).unwrap();
    conn.execute_batch("CREATE TRIGGER fail_metadata BEFORE UPDATE OF last_success_at ON subscription_source BEGIN SELECT RAISE(ABORT,'synthetic failure'); END;").unwrap();
    let stage =
        SubscriptionManager::stage("socks5://new.test:1080#New", SubHeaders::default()).unwrap();
    assert!(db
        .apply_subscription("default", &a, stage, true, &[])
        .is_err());
    assert_eq!(
        serde_json::to_value(Db::new(path.clone()).load_profile("default").unwrap()).unwrap(),
        before
    );
    assert!(
        Db::new(path.clone()).list_subscriptions("default").unwrap()[0]
            .last_success_at
            .is_none()
    );
    conn.execute_batch("DROP TRIGGER fail_metadata").unwrap();
    let stage = SubscriptionManager::stage(LINK, SubHeaders::default()).unwrap();
    let (_, summary) = db
        .apply_subscription("default", &a, stage, true, &[])
        .unwrap();
    let fresh = Db::new(path.clone());
    let actual = fresh
        .list_subscriptions("default")
        .unwrap()
        .into_iter()
        .find(|s| s.id == "a")
        .unwrap();
    assert_eq!(actual.last_success_at, Some(summary.summary.at_unix_ms));
    assert!(actual.unread_summary);
    assert!(fresh
        .list_subscriptions("default")
        .unwrap()
        .into_iter()
        .find(|s| s.id == "b")
        .unwrap()
        .last_success_at
        .is_none());
    let loaded = fresh.load_profile("default").unwrap();
    assert_eq!(loaded.proxies[0].source_id.as_deref(), Some("a"));
    let mut failed = summary.summary.clone();
    failed.at_unix_ms += 1;
    failed.errors = vec!["subscription rejected".into()];
    fresh
        .record_subscription_failure("default", &a, &failed)
        .unwrap();
    let again = Db::new(path.clone());
    let after = again
        .list_subscriptions("default")
        .unwrap()
        .into_iter()
        .find(|s| s.id == "a")
        .unwrap();
    assert_eq!(after.last_success_at, actual.last_success_at);
    assert_eq!(after.last_attempt_at, Some(failed.at_unix_ms));
    assert_eq!(
        serde_json::to_value(again.load_profile("default").unwrap()).unwrap(),
        serde_json::to_value(loaded).unwrap()
    );
    assert_eq!(
        conn.prepare("PRAGMA foreign_key_check")
            .unwrap()
            .query_map([], |_| Ok(()))
            .unwrap()
            .count(),
        0
    );
}

#[test]
fn candidate_validation_rejects_cycles_and_invalid_weights_before_mutation() {
    let mut p = profile();
    let mut policy = p.policies[0].clone();
    policy.tag = "user".into();
    policy.members = Some(vec!["user".into()]);
    p.policies.push(policy);
    let before = serde_json::to_value(&p).unwrap();
    let staged = SubscriptionManager::stage(LINK, SubHeaders::default()).unwrap();
    assert!(SubscriptionManager::reconcile(&mut p, &source("a"), staged).is_err());
    assert_eq!(serde_json::to_value(&p).unwrap(), before);
    p.policies[1].members = Some(vec!["saved".into()]);
    p.policies[1].weights = Some(vec![1, 2]);
    let staged = SubscriptionManager::stage(LINK, SubHeaders::default()).unwrap();
    assert!(SubscriptionManager::reconcile(&mut p, &source("a"), staged).is_err());
}

#[test]
fn every_authored_dependency_retains_tombstone_and_duplicate_existing_tags() {
    let mut p = profile();
    p.policies.clear();
    let mut duplicate = p.proxies[0].clone();
    duplicate.tag = "duplicate".into();
    p.proxies.push(duplicate);
    let mut chain = p.proxies[0].clone();
    chain.origin = ProxyOrigin::Manual;
    chain.source_id = None;
    chain.tag = "chain".into();
    chain.protocol = "chain".into();
    chain.config = Some(
        json!({"type":"chain","tag":"chain","name":"chain","proxies":["duplicate"],"address":"","port":0}),
    );
    p.proxies.push(chain);
    p.dns = Some(DnsConfig {
        servers: Some(vec![DnsServer {
            tag: "dns".into(),
            name: "DNS".into(),
            address: "1.1.1.1".into(),
            detour: Some("saved".into()),
        }]),
        ..Default::default()
    });
    let stage = SubscriptionManager::stage(LINK, SubHeaders::default()).unwrap();
    SubscriptionManager::reconcile(&mut p, &source("a"), stage).unwrap();
    assert!(
        p.proxies.iter().any(|p| p.tag == "duplicate"),
        "existing duplicate referenced tag must not vanish"
    );
    let stage =
        SubscriptionManager::stage("socks5://new.test:1080#New", SubHeaders::default()).unwrap();
    SubscriptionManager::reconcile(&mut p, &source("a"), stage).unwrap();
    assert!(
        !p.proxies
            .iter()
            .find(|p| p.tag == "saved")
            .unwrap()
            .available
    );
    assert!(
        !p.proxies
            .iter()
            .find(|p| p.tag == "duplicate")
            .unwrap()
            .available
    );
}

#[test]
fn legacy_manager_callers_use_atomic_source_apply_and_never_rename_profile() {
    let code = include_str!("../src/core/manager.rs");
    let create = code
        .split("pub fn profile_create(")
        .nth(1)
        .unwrap()
        .split("pub fn profile_update(")
        .next()
        .unwrap();
    let refresh = code
        .split("fn apply_sync_result(")
        .nth(1)
        .unwrap()
        .split("pub fn profile_mark_summary_read(")
        .next()
        .unwrap();
    assert!(!create.contains("meta.name = title"));
    assert!(refresh.contains("apply_subscription("));
    assert!(!refresh.contains("save_profile("));
    assert!(!refresh.contains("save_subscription_info("));
}

#[test]
fn root_registry_does_not_pin_unselected_removed_nodes_or_break_validation() {
    let mut p = profile();
    let mut old = p.proxies[0].clone();
    old.tag = "unselected".into();
    old.link = "socks5://old.test:1080#Old".into();
    old.config = None;
    p.proxies.push(old);
    p.normalize_policy_graph();
    let stage = SubscriptionManager::stage(LINK, SubHeaders::default()).unwrap();
    SubscriptionManager::reconcile(&mut p, &source("a"), stage).unwrap();
    assert!(!p.proxies.iter().any(|p| p.tag == "unselected"));
}

#[test]
fn base64_grammar_accepts_wrapped_and_unpadded_but_rejects_corrupt_padding() {
    use base64::Engine;
    let encoded = base64::engine::general_purpose::STANDARD.encode(LINK);
    let wrapped = encoded
        .as_bytes()
        .chunks(16)
        .map(|c| std::str::from_utf8(c).unwrap())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(SubscriptionManager::stage(&wrapped, SubHeaders::default()).is_ok());
    assert!(
        SubscriptionManager::stage(encoded.trim_end_matches('='), SubHeaders::default()).is_ok()
    );
    assert!(SubscriptionManager::stage(&format!("{encoded}===="), SubHeaders::default()).is_err());
}

#[test]
fn atomic_apply_validates_raw_authored_graph_instead_of_normalizing_away_errors() {
    use zeytun_lib::core::storage::{Db, Storage};
    let path = std::env::temp_dir().join(format!("raw-graph-{}.db", uuid::Uuid::new_v4()));
    let db = Db::new(path.clone());
    db.save_profile(&ZeytunCore::default()).unwrap();
    let a = source("a");
    db.create_subscription("default", &a).unwrap();
    let mut p = profile();
    let mut invalid = p.policies[0].clone();
    invalid.tag = "cycle".into();
    invalid.members = Some(vec!["cycle".into()]);
    invalid.selected_member_tag = Some("cycle".into());
    p.policies.push(invalid);
    db.save_profile(&p).unwrap();
    let staged = SubscriptionManager::stage(LINK, SubHeaders::default()).unwrap();
    assert!(db
        .apply_subscription("default", &a, staged, false, &[])
        .is_err());
    let conn = rusqlite::Connection::open(path).unwrap();
    assert_eq!(
        conn.query_row(
            "SELECT member_tag FROM policy_member WHERE policy_tag='cycle'",
            [],
            |r| r.get::<_, String>(0)
        )
        .unwrap(),
        "cycle"
    );
}

struct DeleteFixture {
    db: zeytun_lib::core::storage::Db,
    dir: std::path::PathBuf,
}
impl DeleteFixture {
    fn new() -> Self {
        use zeytun_lib::core::storage::{Db, Storage};
        let dir = std::env::temp_dir().join(format!("source-delete-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&dir).unwrap();
        let db = Db::new(dir.join("fixture.db"));
        db.save_profile(&ZeytunCore::default()).unwrap();
        db.create_subscription("default", &source("a")).unwrap();
        db.create_subscription("default", &source("b")).unwrap();
        db.save_profile(&delete_profile()).unwrap();
        Self { db, dir }
    }
}
impl Drop for DeleteFixture {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.dir).unwrap();
    }
}
fn delete_profile() -> ZeytunCore {
    let mut p = profile();
    let mut policy = p.policies[0].clone();
    policy.tag = "authored".into();
    policy.members = Some(vec!["saved".into()]);
    policy.selected_member_tag = Some("saved".into());
    policy.weights = Some(vec![7]);
    p.policies.push(policy);
    let mut chain = p.proxies[0].clone();
    chain.tag = "chain".into();
    chain.origin = ProxyOrigin::Manual;
    chain.source_id = None;
    chain.protocol = "chain".into();
    chain.config = Some(json!({"type":"chain","tag":"chain","name":"Chain","proxies":["saved"]}));
    p.proxies.push(chain);
    let mut detour = p.proxies[0].clone();
    detour.tag = "detour".into();
    detour.origin = ProxyOrigin::Manual;
    detour.source_id = None;
    detour.config.as_mut().unwrap()["tag"] = json!("detour");
    detour.config.as_mut().unwrap()["detour"] = json!("saved");
    p.proxies.push(detour);
    p.rules[0].outbound = "saved".into();
    p.rules.push(serde_json::from_value(json!({"id":2,"kind":"DOMAIN","value":"example.test","outbound":"saved","enabled":false,"comment":""})).unwrap());
    p.temp_rules.push(serde_json::from_value(json!({"id":3,"kind":"DOMAIN","value":"temp.test","outbound":"saved","enabled":true,"comment":"","expires_at":i64::MAX})).unwrap());
    let mut session = p.temp_rules[0].clone();
    session.id = 4;
    session.session = true;
    session.expires_at = 0;
    p.session_rules.push(session);
    p.dns = Some(DnsConfig {
        servers: Some(vec![DnsServer {
            tag: "dns".into(),
            name: "DNS".into(),
            address: "1.1.1.1".into(),
            detour: Some("saved".into()),
        }]),
        ..Default::default()
    });
    p.rule_sets=Some(vec![serde_json::from_value(json!({"id":"rs","tag":"rs","type":"remote","source":"https://example.test/rules.srs","action":"saved","download_policy":"saved","enabled":true})).unwrap()]);
    p
}
#[test]
fn delete_preview_walks_all_typed_dependencies_without_writes_or_secrets() {
    use zeytun_lib::core::storage::{Db, Storage};
    let f = DeleteFixture::new();
    let before = serde_json::to_value(f.db.load_profile("default").unwrap()).unwrap();
    let preview =
        f.db.subscription_delete_preview("default", "a", &delete_profile().session_rules)
            .unwrap();
    let kinds = preview
        .dependencies
        .iter()
        .map(|r| {
            serde_json::to_value(&r.kind)
                .unwrap()
                .as_str()
                .unwrap()
                .to_string()
        })
        .collect::<std::collections::HashSet<_>>();
    for kind in [
        "policy_member",
        "policy_selection",
        "proxy_chain",
        "proxy_detour",
        "route",
        "final",
        "temp_rule",
        "session_rule",
        "dns_detour",
        "ruleset_action",
        "ruleset_download_policy",
    ] {
        assert!(kinds.contains(kind), "missing {kind}");
    }
    let wire = serde_json::to_string(&preview).unwrap();
    assert!(!wire.contains("secret"));
    assert!(!wire.contains("https://"));
    assert_eq!(
        serde_json::to_value(
            Db::new(f.db.path().to_path_buf())
                .load_profile("default")
                .unwrap()
        )
        .unwrap(),
        before
    );
}

#[test]
fn dependent_delete_requires_confirmed_manual_conversion_and_preserves_intent() {
    use zeytun_lib::core::storage::{Db, Storage};
    let f = DeleteFixture::new();
    let sessions = delete_profile().session_rules;
    let before = f.db.load_profile("default").unwrap();
    let preview =
        f.db.subscription_delete_preview("default", "a", &sessions)
            .unwrap();
    let mut input = DeleteSubscriptionInput {
        profile_id: "default".into(),
        source_id: "a".into(),
        preview_token: preview.preview_token,
        confirmed: false,
        mode: SubscriptionDeleteMode::DeleteNodes,
    };
    assert!(f.db.delete_subscription(&input, &sessions).is_err());
    input.confirmed = true;
    assert!(f.db.delete_subscription(&input, &sessions).is_err());
    let preview =
        f.db.subscription_delete_preview("default", "a", &sessions)
            .unwrap();
    input.preview_token = preview.preview_token;
    input.mode = SubscriptionDeleteMode::KeepAsManual;
    f.db.delete_subscription(&input, &sessions).unwrap();
    let fresh = Db::new(f.db.path().to_path_buf());
    let after = fresh.load_profile("default").unwrap();
    for (old, new) in before.proxies.iter().zip(&after.proxies) {
        assert_eq!(old.tag, new.tag);
        assert_eq!(old.config, new.config);
        assert_eq!(old.enabled, new.enabled);
        assert_eq!(old.link, new.link);
        assert_eq!(new.origin, ProxyOrigin::Manual);
        assert!(new.source_id.is_none());
    }
    assert_eq!(
        serde_json::to_value(&before.policies).unwrap(),
        serde_json::to_value(&after.policies).unwrap()
    );
    assert_eq!(
        serde_json::to_value(&before.rules).unwrap(),
        serde_json::to_value(&after.rules).unwrap()
    );
    assert_eq!(
        serde_json::to_value(&before.temp_rules).unwrap(),
        serde_json::to_value(&after.temp_rules).unwrap()
    );
    assert_eq!(
        serde_json::to_value(&before.dns).unwrap(),
        serde_json::to_value(&after.dns).unwrap()
    );
    assert_eq!(
        serde_json::to_value(&before.rule_sets).unwrap(),
        serde_json::to_value(&after.rule_sets).unwrap()
    );
    assert_eq!(
        fresh
            .list_subscriptions("default")
            .unwrap()
            .iter()
            .map(|s| s.id.as_str())
            .collect::<Vec<_>>(),
        vec!["b"]
    );
    assert!(
        f.db.delete_subscription(&input, &sessions).is_err(),
        "consumed preview cannot replay"
    );
}

fn delete_input(preview: SubscriptionDeletePreview) -> DeleteSubscriptionInput {
    DeleteSubscriptionInput {
        profile_id: preview.profile_id,
        source_id: preview.source_id,
        preview_token: preview.preview_token,
        confirmed: true,
        mode: SubscriptionDeleteMode::KeepAsManual,
    }
}
#[test]
fn delete_preview_rejects_graph_edits_same_writer_external_writer_and_sessions() {
    use zeytun_lib::core::storage::{Db, Storage};
    let f = DeleteFixture::new();
    let sessions = delete_profile().session_rules;
    let input = delete_input(
        f.db.subscription_delete_preview("default", "a", &sessions)
            .unwrap(),
    );
    let mut p = f.db.load_profile("default").unwrap();
    p.rules[1].comment = "graph edit".into();
    f.db.save_profile(&p).unwrap();
    assert!(f
        .db
        .delete_subscription(&input, &sessions)
        .unwrap_err()
        .to_string()
        .contains("stale"));
    let input = delete_input(
        f.db.subscription_delete_preview("default", "a", &sessions)
            .unwrap(),
    );
    let other = Db::new(f.db.path().to_path_buf());
    p.rules[1].comment = "external edit".into();
    other.save_profile(&p).unwrap();
    assert!(f
        .db
        .delete_subscription(&input, &sessions)
        .unwrap_err()
        .to_string()
        .contains("stale"));
    let input = delete_input(
        f.db.subscription_delete_preview("default", "a", &sessions)
            .unwrap(),
    );
    let mut changed = sessions.clone();
    changed[0].outbound = "direct".into();
    assert!(f
        .db
        .delete_subscription(&input, &changed)
        .unwrap_err()
        .to_string()
        .contains("stale"));
    let input = delete_input(
        f.db.subscription_delete_preview("default", "a", &sessions)
            .unwrap(),
    );
    let conn = rusqlite::Connection::open(f.db.path()).unwrap();
    conn.execute(
        "UPDATE route_rule SET comment='undo probe' WHERE profile_id='default'",
        [],
    )
    .unwrap();
    conn.execute(
        "UPDATE route_rule SET comment='' WHERE profile_id='default' AND id=1",
        [],
    )
    .unwrap();
    assert!(f.db.delete_subscription(&input, &sessions).is_err());
    assert_eq!(f.db.list_subscriptions("default").unwrap().len(), 2);
}

#[test]
fn paused_cache_unavailable_manual_conversion_and_delete_rollback_are_safe() {
    use zeytun_lib::core::storage::{Db, Storage};
    let f = DeleteFixture::new();
    let mut p = f.db.load_profile("default").unwrap();
    p.proxies[0].available = false;
    f.db.save_profile(&p).unwrap();
    let before = serde_json::to_value(f.db.load_profile("default").unwrap()).unwrap();
    let mut settings = source("a").settings;
    settings.enabled = false;
    f.db.update_subscription("default", "a", &settings).unwrap();
    assert_eq!(
        serde_json::to_value(f.db.load_profile("default").unwrap()).unwrap(),
        before
    );
    let conn = rusqlite::Connection::open(f.db.path()).unwrap();
    conn.execute_batch("CREATE TRIGGER fail_source_delete BEFORE DELETE ON subscription_source BEGIN SELECT RAISE(ABORT,'synthetic failure'); END;").unwrap();
    let input = delete_input(
        f.db.subscription_delete_preview("default", "a", &[])
            .unwrap(),
    );
    assert!(f.db.delete_subscription(&input, &[]).is_err());
    let fresh = Db::new(f.db.path().to_path_buf());
    assert_eq!(
        serde_json::to_value(fresh.load_profile("default").unwrap()).unwrap(),
        before
    );
    assert_eq!(fresh.list_subscriptions("default").unwrap().len(), 2);
    conn.execute_batch("DROP TRIGGER fail_source_delete")
        .unwrap();
    let input = delete_input(
        f.db.subscription_delete_preview("default", "a", &[])
            .unwrap(),
    );
    f.db.delete_subscription(&input, &[]).unwrap();
    let after = Db::new(f.db.path().to_path_buf())
        .load_profile("default")
        .unwrap();
    let saved = after.proxies.iter().find(|p| p.tag == "saved").unwrap();
    assert!(!saved.enabled);
    assert!(!saved.available);
    assert!(saved.source_id.is_none());
    assert_eq!(saved.origin, ProxyOrigin::Manual);
    assert_eq!(
        conn.prepare("PRAGMA foreign_key_check")
            .unwrap()
            .query_map([], |_| Ok(()))
            .unwrap()
            .count(),
        0
    );
}

#[test]
fn source_delete_manager_uses_scoped_storage_and_never_restarts_or_cleans_files() {
    let code = include_str!("../src/core/manager.rs");
    for name in ["subscription_delete_preview", "subscription_delete"] {
        let body = code
            .split(&format!("pub fn {name}("))
            .nth(1)
            .expect("manager method missing")
            .split("pub fn ")
            .next()
            .unwrap();
        assert!(body
            .split_whitespace()
            .collect::<String>()
            .contains("self.db."));
        assert!(body.contains("session_rules"));
        for forbidden in [
            "reload_if_running",
            "restart_if_running",
            "remove_file",
            "normalize_policy_graph",
        ] {
            assert!(!body.contains(forbidden), "{name}: {forbidden}");
        }
    }
}

#[test]
fn unrelated_profile_metadata_write_does_not_invalidate_delete_graph_preview() {
    let f = DeleteFixture::new();
    let input = delete_input(
        f.db.subscription_delete_preview("default", "a", &[])
            .unwrap(),
    );
    use zeytun_lib::core::storage::Storage;
    let conn = rusqlite::Connection::open(f.db.path()).unwrap();
    conn.execute(
        "UPDATE profile SET name='Display rename' WHERE id='default'",
        [],
    )
    .unwrap();
    assert!(
        f.db.delete_subscription(&input, &[]).is_ok(),
        "only graph/source/session revision should invalidate previews, not unrelated DB writes"
    );
}

#[test]
fn unreferenced_source_delete_only_removes_its_nodes_and_preserves_other_profiles() {
    use zeytun_lib::core::storage::{Db, Storage};
    let f = DeleteFixture::new();
    let mut p = ZeytunCore {
        proxies: profile().proxies,
        ..ZeytunCore::default()
    };
    let mut b = p.proxies[0].clone();
    b.tag = "b".into();
    b.source_id = Some("b".into());
    b.config.as_mut().unwrap()["tag"] = json!("b");
    let mut manual = b.clone();
    manual.tag = "manual".into();
    manual.origin = ProxyOrigin::Manual;
    manual.source_id = None;
    manual.config.as_mut().unwrap()["tag"] = json!("manual");
    p.proxies.extend([b, manual]);
    p.normalize_policy_graph();
    p.policies[0].selected_member_tag = Some("manual".into());
    f.db.save_profile(&p).unwrap();
    let mut meta = f.db.list_profiles().unwrap().remove(0);
    meta.id = "other".into();
    meta.name = "Other".into();
    meta.is_active = false;
    meta.subscription_url = None;
    f.db.create_profile(&meta).unwrap();
    f.db.create_subscription("other", &source("a")).unwrap();
    let mut other = profile();
    other.id = "other".into();
    f.db.save_profile(&other).unwrap();
    let other_before = serde_json::to_value(f.db.load_profile("other").unwrap()).unwrap();
    let preview =
        f.db.subscription_delete_preview("default", "a", &[])
            .unwrap();
    assert!(preview.dependencies.is_empty());
    let mut input = delete_input(preview);
    input.mode = SubscriptionDeleteMode::DeleteNodes;
    f.db.delete_subscription(&input, &[]).unwrap();
    let fresh = Db::new(f.db.path().to_path_buf());
    let after = fresh.load_profile("default").unwrap();
    assert_eq!(
        after
            .proxies
            .iter()
            .map(|p| p.tag.as_str())
            .collect::<Vec<_>>(),
        vec!["b", "manual"]
    );
    assert_eq!(
        serde_json::to_value(fresh.load_profile("other").unwrap()).unwrap(),
        other_before
    );
    assert_eq!(
        fresh
            .list_subscriptions("default")
            .unwrap()
            .iter()
            .map(|s| s.id.as_str())
            .collect::<Vec<_>>(),
        vec!["b"]
    );
    assert_eq!(fresh.list_subscriptions("other").unwrap()[0].id, "a");
}
