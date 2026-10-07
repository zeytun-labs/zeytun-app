use std::path::PathBuf;

use rusqlite::Connection;
use zeytun_lib::core::{
    dto::{ZeytunCore, DEFAULT_PROFILE_ID},
    storage::{Db, Storage},
};

struct TestDb(PathBuf);

impl TestDb {
    fn new() -> Self {
        Self(
            std::env::temp_dir().join(format!("zeytun-profile-ownership-{}", uuid::Uuid::new_v4())),
        )
    }

    fn path(&self) -> PathBuf {
        self.0.join("zeytun.db")
    }
}

impl Drop for TestDb {
    fn drop(&mut self) {
        if self.0.exists() {
            std::fs::remove_dir_all(&self.0).expect("remove owned test directory");
        }
    }
}

#[test]
fn saving_inactive_profile_does_not_replace_global_inbounds() {
    let fixture = TestDb::new();
    let db = Db::new(fixture.path());
    db.load_profile(DEFAULT_PROFILE_ID)
        .expect("initialize synthetic database");
    // Seed the persisted device state independently of the profile writer being tested.
    let connection = Connection::open(fixture.path()).expect("open fixture");
    connection
        .execute_batch(
            "DELETE FROM local_proxy_config;
         INSERT INTO local_proxy_config (
             mode, listen, socks_port, http_port, mixed_port, tun_name, tun_mtu,
             tun_auto_route, system_proxy, connection_ask_enabled,
             connection_ask_timeout_ms, connection_ask_group_by, connection_ask_remember_default
         ) VALUES ('tun', '0.0.0.0', 7070, 7070, 7070, 'test-tun', 1400,
                   1, 1, 1, 42000, 'process_dest', 0);",
        )
        .expect("seed global fixture");
    drop(connection);
    let before = db
        .load_profile(DEFAULT_PROFILE_ID)
        .expect("read device state")
        .local_proxy;
    let profile = ZeytunCore {
        id: "inactive".into(),
        ..ZeytunCore::default()
    };
    db.save_profile(&profile)
        .expect("save inactive routing bundle");
    drop(db);

    let reopened = Db::new(fixture.path());
    let loaded = reopened
        .load_profile("inactive")
        .expect("reopen inactive profile");
    assert_eq!(
        serde_json::to_value(loaded.local_proxy).unwrap(),
        serde_json::to_value(before).unwrap()
    );
    assert_eq!(reopened.active_profile_id().unwrap(), DEFAULT_PROFILE_ID);
    assert_eq!(loaded.id, "inactive");
}

#[test]
fn device_writer_roundtrips_all_inbound_settings_through_fresh_connection() {
    let fixture = TestDb::new();
    let db = Db::new(fixture.path());
    let mut config = ZeytunCore::default().local_proxy;
    config.listen = "0.0.0.0".into();
    config.mixed_port = Some(7070);
    config.http_port = 7070;
    config.socks_port = 7070;
    config.log_level = Some("debug".into());
    config.mode = Some(zeytun_lib::core::dto::InboundMode::Tun);
    config.system_proxy = Some(true);
    config.connection_ask = Some(zeytun_lib::core::dto::ConnectionAskConfig {
        enabled: true,
        timeout_ms: 42000,
        group_by: "process_dest".into(),
        remember_default: false,
    });
    db.save_local_proxy(&config).expect("save global settings");
    drop(db);

    let reopened = Db::new(fixture.path());
    let loaded = reopened.load_profile(DEFAULT_PROFILE_ID).unwrap();
    assert_eq!(
        serde_json::to_value(&loaded.local_proxy).unwrap(),
        serde_json::to_value(&config).unwrap()
    );
    let seeded = reopened.load_profile("new-profile").unwrap();
    assert_eq!(
        serde_json::to_value(seeded.local_proxy).unwrap(),
        serde_json::to_value(&config).unwrap()
    );
}
