use serde_json::json;
use zeytun_lib::core::{
    dto::*,
    events::EventBus,
    manager::CoreManager,
    storage::{Db, Storage},
    subscription::SubHeaders,
};

#[test]
fn manager_records_apply_validation_failure_after_rollback_without_losing_success() {
    let dir = std::env::temp_dir().join(format!("manager-apply-{}", uuid::Uuid::new_v4()));
    let mut manager =
        CoreManager::new(dir.clone(), None, std::sync::Arc::new(EventBus::headless())).unwrap();
    let db = Db::new(dir.join("core/zeytun.db"));
    let source: SubscriptionSource = serde_json::from_value(json!({"id":"a","generation":0,"position":0,"settings":{"name":"a","url":"https://example.test/private","enabled":true,"auto_update":true,"update_interval_hours":24}})).unwrap();
    db.create_subscription("default", &source).unwrap();
    manager
        .profile_apply_refresh(
            "default",
            "socks5://user:secret@example.test:1080#Node",
            SubHeaders::default(),
            false,
        )
        .unwrap();
    let old = db.list_subscriptions("default").unwrap().remove(0);
    let mut p = db.load_profile("default").unwrap();
    let mut cycle = p.policies[0].clone();
    cycle.tag = "cycle".into();
    cycle.members = Some(vec!["cycle".into()]);
    cycle.selected_member_tag = Some("cycle".into());
    p.policies.push(cycle);
    db.save_profile(&p).unwrap();
    let conn = rusqlite::Connection::open(db.path()).unwrap();
    let before: String = conn
        .query_row("SELECT group_concat(config_json) FROM proxy", [], |r| {
            r.get(0)
        })
        .unwrap();
    let before_revision: i64 = conn
        .query_row(
            "SELECT config_revision FROM profile WHERE id='default'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    let result = manager.profile_apply_refresh(
        "default",
        "socks5://new.test:1080#New",
        SubHeaders::default(),
        false,
    );
    let fresh = Db::new(db.path().to_path_buf());
    let after = fresh.list_subscriptions("default").unwrap().remove(0);
    assert!(
        after.last_error.is_some(),
        "apply failure not recorded: {result:?}"
    );
    assert!(after.last_attempt_at.is_some());
    assert_eq!(after.last_success_at, old.last_success_at);
    assert_eq!(
        serde_json::to_value(after.last_sync_summary).unwrap(),
        serde_json::to_value(old.last_sync_summary).unwrap()
    );
    assert_eq!(
        before,
        conn.query_row::<String, _, _>("SELECT group_concat(config_json) FROM proxy", [], |r| r
            .get(0))
            .unwrap()
    );
    assert_eq!(
        before_revision,
        conn.query_row::<i64, _, _>(
            "SELECT config_revision FROM profile WHERE id='default'",
            [],
            |r| r.get(0)
        )
        .unwrap()
    );
    assert!(!after.last_error.unwrap().contains("secret"));
    drop(conn);
    drop(manager);
    drop(db);
    let _ = std::fs::remove_dir_all(dir);
}
