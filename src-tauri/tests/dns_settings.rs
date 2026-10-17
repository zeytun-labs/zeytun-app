use std::{path::PathBuf, sync::Arc};
use zeytun_lib::core::{
    dto::{DnsConfig, DnsRule, DnsServer, UpdateProfileInput},
    events::EventBus,
    manager::CoreManager,
    storage::{Db, Storage},
};

struct Fixture {
    dir: PathBuf,
    manager: CoreManager,
    db: Db,
}
impl Fixture {
    fn new() -> Self {
        let dir = std::env::temp_dir().join(format!("dns-settings-{}", uuid::Uuid::new_v4()));
        let manager = CoreManager::new(dir.clone(), None, Arc::new(EventBus::headless())).unwrap();
        let db = Db::new(dir.join("core/zeytun.db"));
        Self { dir, manager, db }
    }
    fn update(&mut self, dns: DnsConfig) -> Result<(), zeytun_lib::error::CommandError> {
        let input: UpdateProfileInput =
            serde_json::from_value(serde_json::json!({ "id": "default", "dns": dns })).unwrap();
        self.manager.profile_update(input).map(|_| ())
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn dns() -> DnsConfig {
    DnsConfig {
        mode: None,
        servers: Some(vec![DnsServer {
            tag: "google".into(),
            name: "Google".into(),
            address: "https://dns.google/dns-query".into(),
            detour: Some("root-policy".into()),
        }]),
        rules: Some(vec![DnsRule {
            id: "discord".into(),
            kind: "domain".into(),
            value: "updates.discord.com".into(),
            target: "google".into(),
            comment: None,
            enabled: true,
        }]),
        final_server: Some("google".into()),
        fake_ip: None,
        hosts: None,
    }
}

#[test]
fn editing_dns_server_preserves_identity_references_and_profile_reload() {
    let mut f = Fixture::new();
    f.update(dns()).unwrap();
    let revision = f.db.config_revision("default").unwrap();
    let mut update = f.db.load_profile("default").unwrap().dns.unwrap();
    let server = &mut update.servers.as_mut().unwrap()[0];
    server.name = "Renamed".into();
    f.update(update.clone()).unwrap();
    assert_eq!(
        f.db.config_revision("default").unwrap(),
        revision,
        "display rename must not need restart"
    );
    update.servers.as_mut().unwrap()[0].address =
        "https://[2001:4860:4860::8888]:443/dns-query".into();
    f.update(update).unwrap();
    let saved = f.db.load_profile("default").unwrap().dns.unwrap();
    assert_eq!(saved.servers.as_ref().unwrap()[0].tag, "google");
    assert_eq!(saved.rules.as_ref().unwrap()[0].target, "google");
    assert_eq!(saved.final_server.as_deref(), Some("google"));
    assert!(f.db.config_revision("default").unwrap() > revision);
    assert_eq!(
        f.manager
            .profile_get()
            .dns
            .as_ref()
            .unwrap()
            .servers
            .as_ref()
            .unwrap()[0]
            .name,
        "Renamed"
    );
}

#[test]
fn invalid_dns_edit_or_deletion_does_not_change_saved_profile() {
    let mut f = Fixture::new();
    f.update(dns()).unwrap();
    let before = serde_json::to_value(f.db.load_profile("default").unwrap()).unwrap();
    for invalid in [
        "https://user:password@dns.example/dns-query",
        "1.1.1.1:0",
        "udp://[::1]/path",
    ] {
        let mut update = dns();
        update.servers.as_mut().unwrap()[0].address = invalid.into();
        assert!(f.update(update).is_err());
        assert_eq!(
            serde_json::to_value(f.db.load_profile("default").unwrap()).unwrap(),
            before
        );
    }
    let mut update = dns();
    update.servers = None;
    assert!(
        f.update(update).is_err(),
        "referenced resolver deletion must be rejected"
    );
    assert_eq!(
        serde_json::to_value(f.db.load_profile("default").unwrap()).unwrap(),
        before
    );
}

#[test]
fn local_dns_is_an_explicit_rule_target_with_a_remote_final_resolver() {
    let mut f = Fixture::new();
    let mut update = dns();
    update.rules.as_mut().unwrap()[0].target = "local-dns".into();
    f.update(update).unwrap();
    assert_eq!(
        f.db.load_profile("default")
            .unwrap()
            .dns
            .unwrap()
            .rules
            .unwrap()[0]
            .target,
        "local-dns"
    );
}
