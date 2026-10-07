use rusqlite::{params, Connection};
use serde_json::json;
use zeytun_lib::core::{
    dto::*,
    storage::{Db, Storage},
};

fn fixture() -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("subscription-storage-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("fixture.db");
    let conn = Connection::open(&path).unwrap();
    conn.execute_batch("PRAGMA foreign_keys=ON; CREATE TABLE schema_migration(version INTEGER PRIMARY KEY,name TEXT NOT NULL,applied_unix_ms INTEGER NOT NULL);").unwrap();
    let mut files = std::fs::read_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/migrations"))
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|p| p.extension().is_some_and(|s| s == "sql"))
        .collect::<Vec<_>>();
    files.sort();
    for file in files {
        let name = file.file_name().unwrap().to_str().unwrap();
        let version: i64 = name[..4].parse().unwrap();
        if version > 19 {
            continue;
        }
        conn.execute_batch(&std::fs::read_to_string(&file).unwrap())
            .unwrap();
        conn.execute(
            "INSERT INTO schema_migration VALUES (?1,?2,0)",
            params![version, name],
        )
        .unwrap();
    }
    conn.execute_batch("UPDATE profile SET name='Preserved',subscription_url='https://example.test/token',last_updated_unix_ms=17,last_sync_summary='{\"proxies_added\":1,\"proxies_removed\":0,\"proxies_kept\":0,\"rules_orphaned\":0,\"at_unix_ms\":17,\"errors\":[]}',unread_summary=1,skip_auto_update=1,update_interval_hours=24,sub_upload=1,sub_download=2,sub_total=3,sub_expire=4;
        INSERT INTO profile(id,name) VALUES ('orphan','Orphan');
        INSERT INTO proxy(profile_id,tag,origin,title,protocol,link,enabled,position) VALUES ('default','sub','subscription','n','socks','socks5://example.test:1080',0,0),('default','manual','manual','m','socks','',1,1),('orphan','sub','subscription','o','socks','',1,0);
        INSERT INTO policy(profile_id,tag,name,kind,selected_member_tag,position,weights_json) VALUES ('default','user','User','balancer','sub',0,'[7,11]');
        INSERT INTO policy_member VALUES ('default','user','sub',0),('default','user','manual',1);
        INSERT INTO route_rule(profile_id,id,kind,value,outbound,enabled,comment,position) VALUES ('default',0,'FINAL','','sub',1,'keep',0);").unwrap();
    path
}

#[test]
fn schema19_migration_preserves_graph_and_backfills_ownership() {
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
    assert_eq!(
        conn.query_row(
            "SELECT subscription_url FROM profile WHERE id='default'",
            [],
            |r| r.get::<_, Option<String>>(0)
        )
        .unwrap(),
        None
    );
}

fn source(id: &str) -> SubscriptionSource {
    serde_json::from_value(json!({"id":id,"settings":{"name":"A","url":"https://example.test/private?token=secret","enabled":true,"auto_update":true,"update_interval_hours":24},"generation":2,"position":3,
        "provider_title":"Provider","suggested_interval_hours":48,"userinfo":{"upload":1,"download":2,"total":0,"expire":0},"last_attempt_at":51,"last_success_at":50,"last_error":"safe error","last_sync_summary":{"proxies_added":1,"proxies_removed":0,"proxies_kept":1,"rules_orphaned":0,"at_unix_ms":50,"errors":[]},"unread_summary":true})).unwrap()
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
    let conn = Connection::open(path).unwrap();
    assert_eq!(
        conn.query_row(
            "SELECT subscription_url FROM profile WHERE id='default'",
            [],
            |r| r.get::<_, Option<String>>(0)
        )
        .unwrap(),
        None
    );
}

#[test]
fn failed_schema19_migration_rolls_back_ddl_data_and_marker() {
    let path = fixture();
    let conn = Connection::open(&path).unwrap();
    conn.execute("UPDATE policy SET weights_json='[7]' WHERE tag='user'", [])
        .unwrap();
    assert!(Db::new(path.clone()).list_profiles().is_err());
    assert_eq!(
        conn.query_row(
            "SELECT count(*) FROM sqlite_master WHERE name='subscription_source'",
            [],
            |r| r.get::<_, i64>(0)
        )
        .unwrap(),
        0
    );
    assert_eq!(
        conn.query_row(
            "SELECT count(*) FROM schema_migration WHERE version=20",
            [],
            |r| r.get::<_, i64>(0)
        )
        .unwrap(),
        0
    );
    assert_eq!(
        conn.query_row("SELECT count(*) FROM proxy", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        3
    );
    conn.execute(
        "UPDATE policy SET weights_json='[7,11]' WHERE tag='user'",
        [],
    )
    .unwrap();
    assert!(Db::new(path).list_profiles().is_ok());
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
