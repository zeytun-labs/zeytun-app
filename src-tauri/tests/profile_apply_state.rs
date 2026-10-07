use std::sync::Arc;
use zeytun_lib::core::{
    dto::*,
    events::EventBus,
    manager::CoreManager,
    storage::{Db, Storage},
    subscription::SubHeaders,
};
struct Fixture {
    dir: std::path::PathBuf,
    manager: CoreManager,
    db: Db,
}
impl Fixture {
    fn new() -> Self {
        let dir = std::env::temp_dir().join(format!("apply-state-{}", uuid::Uuid::new_v4()));
        let manager = CoreManager::new(dir.clone(), None, Arc::new(EventBus::headless())).unwrap();
        let db = Db::new(dir.join("core/zeytun.db"));
        Self { dir, manager, db }
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.dir).unwrap();
    }
}
#[test]
fn saved_revision_tracks_runtime_changes_not_titles_or_noop_saves() {
    let mut f = Fixture::new();
    let initial = f.manager.saved_revision("default").unwrap();
    f.manager.set_listen_port(7070).unwrap();
    let changed = f.manager.saved_revision("default").unwrap();
    assert!(
        changed > initial,
        "device port did not advance saved revision"
    );
    f.manager.set_listen_port(7070).unwrap();
    assert_eq!(f.manager.saved_revision("default").unwrap(), changed);
    let node = f
        .manager
        .proxy_import_link("socks5://node.test:1080#Node")
        .unwrap();
    let changed = f.manager.saved_revision("default").unwrap();
    let mut p = f.db.load_profile("default").unwrap();
    p.proxies
        .iter_mut()
        .find(|n| n.tag == node.tag)
        .unwrap()
        .title = "Display only".into();
    f.db.save_profile(&p).unwrap();
    assert_eq!(f.manager.saved_revision("default").unwrap(), changed);
}
#[test]
fn subscription_refresh_is_persist_only_and_metadata_only_does_not_advance_revision() {
    let mut f = Fixture::new();
    let source:SubscriptionSource=serde_json::from_value(serde_json::json!({"id":"a","generation":0,"position":0,"settings":{"name":"a","url":"https://example.test/private","enabled":true,"auto_update":false,"update_interval_hours":24}})).unwrap();
    f.db.create_subscription("default", &source).unwrap();
    f.manager
        .profile_apply_refresh(
            "default",
            "socks5://node.test:1080#First",
            SubHeaders::default(),
            false,
        )
        .unwrap();
    let revision = f.manager.saved_revision("default").unwrap();
    f.manager
        .profile_apply_refresh(
            "default",
            "socks5://node.test:1080#Renamed",
            SubHeaders::default(),
            false,
        )
        .unwrap();
    assert_eq!(f.manager.saved_revision("default").unwrap(), revision);
    let code = include_str!("../src/core/manager.rs");
    let apply = code
        .split("fn apply_sync_result(")
        .nth(1)
        .unwrap()
        .split("pub fn profile_mark_summary_read(")
        .next()
        .unwrap();
    assert!(
        !apply.contains("reload_if_running"),
        "source refresh auto-applies saved graph"
    );
    assert!(!f.manager.is_running());
}
#[test]
fn local_proxy_download_while_stopped_errors_instead_of_falling_back_direct() {
    let mut f = Fixture::new();
    f.manager
        .set_network_policy(
            "subscription",
            &zeytun_lib::core::network_policy::NetworkPolicy::LocalProxy,
        )
        .unwrap();
    assert!(f.manager.proxy_for_traffic("subscription").is_err());
}

#[test]
fn failed_start_never_claims_applied_revision_and_preserves_pending() {
    let mut f = Fixture::new();
    f.manager.set_listen_port(7070).unwrap();
    assert!(f.manager.start().is_err()); // No binary in this synthetic fixture; no process/network mutations.
    let status = f.manager.status();
    assert!(status.applied_revision.is_none());
    assert!(status.pending_apply);
    assert_eq!(
        status.saved_revision,
        f.manager.saved_revision("default").unwrap()
    );
    assert!(status.applied_mixed_port.is_none());
}
