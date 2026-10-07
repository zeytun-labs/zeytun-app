use rusqlite::Connection;
use serde_json::json;
use zeytun_lib::core::{
    dto::*,
    storage::{Db, Storage},
};

fn fixture() -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("subscription-storage-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("fixture.db");
    let db = Db::new(path.clone());

    let mut p = db.load_profile("default").unwrap();
    let source = SubscriptionSource {
        id: "legacy".into(),
        settings: SubscriptionSettings {
            name: "Preserved".into(),
            url: "https://example.test/token".into(),
            enabled: true,
            auto_update: false,
            update_interval_hours: 24,
        },
        generation: 0,
        position: 0,
        provider_title: None,
        suggested_interval_hours: None,
        userinfo: Some(SubUserinfo {
            upload: 1,
            download: 2,
            total: 3,
            expire: 4,
        }),
        last_attempt_at: Some(17),
        last_success_at: Some(17),
        last_error: None,
        last_sync_summary: Some(SyncSummary {
            proxies_added: 1,
            proxies_removed: 0,
            proxies_kept: 0,
            rules_orphaned: 0,
            at_unix_ms: 17,
            errors: vec![],
            effective_fallbacks: vec![],
        }),
        unread_summary: true,
    };
    db.create_subscription("default", &source).unwrap();

    let sub_node = Proxy {
        tag: "sub".into(),
        origin: ProxyOrigin::Subscription,
        source_id: Some("legacy".into()),
        available: true,
        title: "n".into(),
        protocol: "socks".into(),
        link: "socks5://example.test:1080".into(),
        enabled: false,
        transport: None,
        config: None,
    };
    let manual_node = Proxy {
        tag: "manual".into(),
        origin: ProxyOrigin::Manual,
        source_id: None,
        available: true,
        title: "m".into(),
        protocol: "socks".into(),
        link: String::new(),
        enabled: true,
        transport: None,
        config: None,
    };
    p.proxies = vec![sub_node, manual_node];

    let mut policy = p.policies[0].clone();
    policy.tag = "user".into();
    policy.name = "User".into();
    policy.kind = Some(ProxyPolicyType::Balancer);
    policy.members = Some(vec!["sub".into(), "manual".into()]);
    policy.weights = Some(vec![7, 11]);
    policy.selected_member_tag = Some("sub".into());
    p.policies.push(policy);

    p.rules = vec![Rule {
        id: 0,
        kind: RuleType::Final,
        value: String::new(),
        outbound: "sub".into(),
        enabled: true,
        comment: "keep".into(),
        rule_set: None,
        orphaned: false,
    }];
    db.save_profile(&p).unwrap();

    let orphan = ProfileMeta {
        id: "orphan".into(),
        name: "Orphan".into(),
        is_active: false,
        icon: None,
        subscription_url: None,
        last_updated_unix_ms: None,
        last_sync_summary: None,
        unread_summary: false,
        skip_auto_update: true,
        update_interval_hours: 12,
        sub_upload: None,
        sub_download: None,
        sub_total: None,
        sub_expire: None,
    };
    db.create_profile(&orphan).unwrap();
    let mut orphan_p = db.load_profile("orphan").unwrap();
    orphan_p.proxies = vec![Proxy {
        tag: "sub".into(),
        origin: ProxyOrigin::Manual,
        source_id: None,
        available: true,
        title: "o".into(),
        protocol: "socks".into(),
        link: String::new(),
        enabled: true,
        transport: None,
        config: None,
    }];
    db.save_profile(&orphan_p).unwrap();

    path
}

#[test]
fn clean_schema_initializes_and_preserves_graph_ownership() {
    let path = fixture();
    let db = Db::new(path.clone());
    let p = db.load_profile("default").unwrap();
    let sub = p.proxies.iter().find(|p| p.tag == "sub").unwrap();
    assert_eq!(sub.source_id.as_deref(), Some("legacy"));
    assert!(!sub.enabled);
    assert!(sub.available);
    let policy = p.policies.iter().find(|p| p.tag == "user").unwrap();
    assert_eq!(policy.members.as_ref().unwrap(), &["sub", "manual"]);
    assert_eq!(policy.weights.as_ref().unwrap(), &[7, 11]);
    assert_eq!(policy.selected_member_tag.as_deref(), Some("sub"));
    assert_eq!(p.final_outbound(), "sub");
    assert_eq!(
        db.load_profile("orphan").unwrap().proxies[0].origin,
        ProxyOrigin::Manual
    );
    db.save_profile(&p).unwrap();
    let reopened = Db::new(path.clone());
    assert_eq!(
        reopened.load_profile("default").unwrap().proxies[0]
            .source_id
            .as_deref(),
        Some("legacy")
    );
    assert_eq!(reopened.active_profile_id().unwrap(), "default");
    let conn = Connection::open(path).unwrap();
    let count: i64 = conn
        .query_row("SELECT count(*) FROM subscription_source", [], |r| r.get(0))
        .unwrap();
    assert_eq!(count, 1);
    assert_eq!(
        conn.query_row("SELECT last_success_at FROM subscription_source", [], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        17
    );
    let fk = conn
        .prepare("PRAGMA foreign_key_check")
        .unwrap()
        .query_map([], |_| Ok(()))
        .unwrap()
        .count();
    assert_eq!(fk, 0);
    let columns = conn
        .prepare("PRAGMA table_info(policy)")
        .unwrap()
        .query_map([], |r| r.get::<_, String>(1))
        .unwrap()
        .map(Result::unwrap)
        .collect::<Vec<_>>();
    assert!(!columns.iter().any(|c| c.contains("source")));
}

fn source(id: &str) -> SubscriptionSource {
    serde_json::from_value(json!({"id":id,"settings":{"name":"A","url":"https://example.test/private?token=secret","enabled":true,"auto_update":true,"update_interval_hours":24},"generation":2,"position":3,
        "provider_title":"Provider","suggested_interval_hours":48,"userinfo":{"upload":1,"download":2,"total":0,"expire":0},"last_attempt_at":51,"last_success_at":50,"last_error":"safe error","last_sync_summary":{"proxies_added":1,"proxies_removed":0,"proxies_kept":1,"rules_orphaned":0,"at_unix_ms":50,"errors":[],"effective_fallbacks":[]},"unread_summary":true})).unwrap()
}

#[test]
fn sources_and_provenance_roundtrip_and_pause_preserves_cache() {
    let path = fixture();
    let db = Db::new(path.clone());
    db.load_profile("default").unwrap();
    let s = source("a");
    db.create_subscription("default", &s).unwrap();
    let mut p = db.load_profile("default").unwrap();
    p.proxies[0].source_id = Some("a".into());
    p.proxies[0].available = false;
    db.save_profile(&p).unwrap();
    let fresh = Db::new(path.clone());
    let actual = fresh
        .list_subscriptions("default")
        .unwrap()
        .into_iter()
        .find(|s| s.id == "a")
        .unwrap();
    assert_eq!(
        serde_json::to_value(&actual).unwrap(),
        serde_json::to_value(&s).unwrap()
    );
    let mut paused = s.settings.clone();
    paused.enabled = false;
    fresh.update_subscription("default", "a", &paused).unwrap();
    let again = Db::new(path.clone());
    let after = again
        .list_subscriptions("default")
        .unwrap()
        .into_iter()
        .find(|s| s.id == "a")
        .unwrap();
    assert_eq!(after.generation, 3);
    assert_eq!(after.last_success_at, Some(50));
    assert_eq!(
        serde_json::to_value(again.load_profile("default").unwrap().proxies).unwrap(),
        serde_json::to_value(p.proxies).unwrap()
    );
    again.update_subscription("default", "a", &paused).unwrap();
    assert_eq!(
        again
            .list_subscriptions("default")
            .unwrap()
            .into_iter()
            .find(|s| s.id == "a")
            .unwrap()
            .generation,
        3
    );
    let mut invalid = paused;
    invalid.url = "file:///private/secret?token=secret".into();
    let error = again
        .update_subscription("default", "a", &invalid)
        .unwrap_err()
        .to_string();
    assert!(!error.contains("secret"));
    assert!(again.create_subscription("missing", &s).is_err());
    let mut cross = again.load_profile("orphan").unwrap();
    cross.proxies[0].origin = ProxyOrigin::Subscription;
    cross.proxies[0].source_id = Some("a".into());
    assert!(again.save_profile(&cross).is_err());
    assert_eq!(
        Db::new(path.clone())
            .load_profile("orphan")
            .unwrap()
            .proxies[0]
            .origin,
        ProxyOrigin::Manual
    );
    let conn = Connection::open(path).unwrap();
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
fn legacy_adapters_use_source_storage_not_profile_columns() {
    let path = fixture();
    let db = Db::new(path.clone());
    let mut meta = db
        .list_profiles()
        .unwrap()
        .into_iter()
        .find(|p| p.id == "default")
        .unwrap();
    assert_eq!(
        meta.subscription_url.as_deref(),
        Some("https://example.test/token")
    );
    assert_eq!(meta.sub_download, Some(2));
    meta.name = "Renamed profile".into();
    meta.update_interval_hours = 30;
    db.update_profile_meta(&meta).unwrap();
    db.save_subscription_info(
        "default",
        Some(SubUserinfo {
            upload: 9,
            download: 10,
            total: 0,
            expire: 0,
        }),
        Some(72),
    )
    .unwrap();
    let mut summary = meta.last_sync_summary.unwrap();
    summary.at_unix_ms = 123;
    summary.errors = vec!["parse failed".into()];
    db.save_sync_summary("default", &summary, true).unwrap();
    let fresh = Db::new(path.clone());
    let source = fresh.list_subscriptions("default").unwrap().remove(0);
    assert_eq!(source.settings.update_interval_hours, 30);
    assert_eq!(source.suggested_interval_hours, Some(72));
    assert_eq!(source.last_success_at, Some(17));
    assert_eq!(source.last_attempt_at, Some(123));
    assert_eq!(source.last_error.as_deref(), Some("parse failed"));
    assert!(
        source.last_sync_summary.as_ref().unwrap().errors.is_empty(),
        "failed attempt must retain the previous accepted summary"
    );
    assert_eq!(source.userinfo.unwrap().download, 10);
    fresh.mark_summary_read("default").unwrap();
    assert!(!fresh.list_subscriptions("default").unwrap()[0].unread_summary);
}

#[test]
fn single_migration_baseline_records_init() {
    let path = fixture();
    let conn = Connection::open(path).unwrap();
    let count: i64 = conn
        .query_row("SELECT count(*) FROM schema_migration", [], |r| r.get(0))
        .unwrap();
    assert_eq!(count, 1);
    let name: String = conn
        .query_row(
            "SELECT name FROM schema_migration WHERE version=1",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(name, "init");
}

#[test]
fn renaming_profile_through_legacy_adapter_does_not_change_paused_source_settings() {
    let path = fixture();
    let db = Db::new(path);
    let s = db.list_subscriptions("default").unwrap().remove(0);
    let mut paused = s.settings;
    paused.enabled = false;
    paused.auto_update = true;
    db.update_subscription("default", "legacy", &paused)
        .unwrap();
    let mut meta = db
        .list_profiles()
        .unwrap()
        .into_iter()
        .find(|p| p.id == "default")
        .unwrap();
    meta.name = "Changed".into();
    db.update_profile_meta(&meta).unwrap();
    let actual = db.list_subscriptions("default").unwrap().remove(0);
    assert_eq!(actual.settings, paused);
    assert_eq!(actual.generation, 1);
}
