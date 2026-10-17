use std::sync::Arc;
use zeytun_lib::core::{
    dto::*,
    events::EventBus,
    manager::CoreManager,
    storage::{Db, Storage},
    subscription::SubHeaders,
};
struct TempDir(std::path::PathBuf);
impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

struct Fixture {
    manager: CoreManager,
    db: Db,
    _dir: TempDir,
}
impl Fixture {
    fn new() -> Self {
        let dir = std::env::temp_dir().join(format!("apply-state-{}", uuid::Uuid::new_v4()));
        let manager = CoreManager::new(dir.clone(), None, Arc::new(EventBus::headless())).unwrap();
        let db = Db::new(dir.join("core/zeytun.db"));
        Self {
            manager,
            db,
            _dir: TempDir(dir),
        }
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

#[test]
fn selecting_proxy_member_or_mode_does_not_advance_saved_revision() {
    let mut f = Fixture::new();
    let initial = f.manager.saved_revision("default").unwrap();
    let p1 = f
        .manager
        .proxy_import_link("socks5://node1.test:1080#N1")
        .unwrap();
    let p2 = f
        .manager
        .proxy_import_link("socks5://node2.test:1080#N2")
        .unwrap();
    let rev_after_import = f.manager.saved_revision("default").unwrap();
    assert!(rev_after_import > initial);

    let pol = f
        .manager
        .create_policy(CreatePolicyInput {
            name: "Group".into(),
            kind: ProxyPolicyType::Selector,
            members: vec![p1.tag.clone(), p2.tag.clone()],
            selected_member_tag: Some(p1.tag.clone()),
            tolerance_ms: None,
            test_url: None,
            interval_seconds: None,
            strategy: None,
            weights: None,
        })
        .unwrap();
    let rev_after_policy = f.manager.saved_revision("default").unwrap();
    assert!(rev_after_policy > rev_after_import);

    // Switching selected member must NOT bump revision
    f.manager
        .select_policy_member(SelectPolicyMemberInput {
            policy_tag: pol.tag.clone(),
            member_tag: p2.tag.clone(),
        })
        .unwrap();

    assert_eq!(
        f.manager.saved_revision("default").unwrap(),
        rev_after_policy,
        "switching policy member must not advance saved revision"
    );

    // Switching outbound mode must NOT bump revision
    f.manager.set_mode(OutboundMode::Global).unwrap();
    assert_eq!(
        f.manager.saved_revision("default").unwrap(),
        rev_after_policy,
        "switching outbound mode must not advance saved revision"
    );

    f.manager.set_mode(OutboundMode::Rule).unwrap();
    assert_eq!(
        f.manager.saved_revision("default").unwrap(),
        rev_after_policy,
        "switching outbound mode to rule must not advance saved revision"
    );
}

#[test]
fn unchanged_background_refresh_preserves_registry_order_and_revision() {
    let f = Fixture::new();
    let source = |id: &str| -> SubscriptionSource {
        serde_json::from_value(serde_json::json!({
            "id": id, "generation": 0, "position": 0,
            "settings": {"name": id, "url": "https://example.test/sub",
                "enabled": true, "auto_update": true, "update_interval_hours": 24}
        }))
        .unwrap()
    };
    let a = source("a");
    let b = source("b");
    for s in [&a, &b] {
        f.db.create_subscription("default", s).unwrap();
    }
    let refresh = |s: &SubscriptionSource, body: &str| {
        let staged =
            zeytun_lib::core::subscription::SubscriptionManager::stage(body, SubHeaders::default())
                .unwrap();
        f.db.apply_subscription("default", s, staged, true, &[])
            .unwrap()
    };
    refresh(&a, "socks5://a1.test:1080#A1\nsocks5://a2.test:1080#A2");
    refresh(&b, "socks5://b.test:1080#B");
    let before = f.db.load_profile("default").unwrap();
    let revision = f.db.config_revision("default").unwrap();

    // Repeated source refreshes must not move one source behind another, even
    // when identical endpoints arrive in a different order or with new titles.
    for (s, body, kept) in [
        (&a, "socks5://a1.test:1080#A1\nsocks5://a2.test:1080#A2", 2),
        (
            &a,
            "socks5://a2.test:1080#Renamed A2\nsocks5://a1.test:1080#A1",
            2,
        ),
        (&b, "socks5://b.test:1080#B", 1),
    ] {
        let (after, result) = refresh(s, body);
        assert_eq!(result.summary.proxies_added, 0);
        assert_eq!(result.summary.proxies_removed, 0);
        assert_eq!(result.summary.proxies_kept, kept);
        assert_eq!(f.db.config_revision("default").unwrap(), revision);
        assert_eq!(
            after.proxies.iter().map(|p| &p.tag).collect::<Vec<_>>(),
            before.proxies.iter().map(|p| &p.tag).collect::<Vec<_>>(),
            "a no-op source refresh must not reshuffle the unified registry"
        );
        assert_eq!(
            serde_json::to_value(&after.policies).unwrap(),
            serde_json::to_value(&before.policies).unwrap()
        );
    }

    refresh(
        &a,
        "socks5://a1.test:1080#A1\nsocks5://a2.test:1080#A2\nsocks5://new.test:1080#New",
    );
    assert!(
        f.db.config_revision("default").unwrap() > revision,
        "a new endpoint still requires applying the saved graph"
    );
}
