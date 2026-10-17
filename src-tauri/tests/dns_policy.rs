use serde_json::{json, Value};
use zeytun_lib::core::{
    compiler::{
        profile_converter::ProfileConverter, runtime_profile_compiler::RuntimeProfileCompiler,
    },
    dto::*,
    link_parser::proxy_from_link,
    profile_document::{ExportMode, ProfileDocument},
    storage::{Db, Storage},
    zeytun_core_config::{dns_direct_tag, ZeytunCoreConfigBuilder},
};

fn profile() -> ZeytunCore {
    let mut profile = ZeytunCore::default();
    profile.proxies.push(
        proxy_from_link(
            "socks5://proxy.example:1080",
            ProxyOrigin::Manual,
            "node".into(),
        )
        .unwrap(),
    );
    profile.normalize_policy_graph();
    profile.dns = Some(DnsConfig::with_default_server());
    profile
}

fn config(profile: &ZeytunCore, mode: OutboundMode) -> Value {
    let runtime = RuntimeProfileCompiler::new()
        .compile(&ProfileConverter::convert(profile))
        .unwrap();
    ZeytunCoreConfigBuilder::new(true, false)
        .build(&runtime, &mode)
        .unwrap()
}

fn server<'a>(config: &'a Value, tag: &str) -> &'a Value {
    config["dns"]["servers"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["tag"] == tag)
        .unwrap()
}

#[test]
fn legacy_defaults_become_editable_servers_without_changing_authored_routes() {
    for raw in [
        json!({}),
        json!({"final_server":"local-dns"}),
        json!({"mode":"automatic"}),
    ] {
        let mut dns: DnsConfig = serde_json::from_value(raw).unwrap();
        dns.migrate_legacy();
        assert_eq!(dns.mode, None);
        assert_eq!(dns.final_server.as_deref(), Some("dns-default"));
        let server = &dns.servers.unwrap()[0];
        assert_eq!(server.detour.as_deref(), Some("follow-mode"));
    }
    let mut dns: DnsConfig = serde_json::from_value(json!({
        "mode":"automatic", "final_server":"dns-default", "servers":[
            {"tag":"dns-default","name":"My DNS","address":"1.1.1.1"}
        ]
    }))
    .unwrap();
    dns.migrate_legacy();
    assert_eq!(dns.final_server.as_deref(), Some("dns-default-1"));
    assert_eq!(dns.servers.as_ref().unwrap()[0].name, "My DNS");
    assert_eq!(
        dns.servers.as_ref().unwrap()[0].detour.as_deref(),
        Some("direct")
    );
    assert!(serde_json::to_value(&dns).unwrap().get("mode").is_none());
}

#[test]
fn follow_mode_dns_uses_selected_endpoint_and_live_proxy_policy() {
    let p = profile();
    let cfg = config(&p, OutboundMode::Global);
    let proxy = server(&cfg, "dns-default");
    assert_eq!(proxy["type"], "https");
    assert_eq!(proxy["server"], "8.8.8.8");
    assert_eq!(proxy["path"], "/dns-query");
    // HTTPS verifies the endpoint certificate; no provider-specific TLS override.
    assert!(proxy["tls"].get("insecure").is_none());
    let global = cfg["route"]["rules"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["clash_mode"] == "global")
        .unwrap();
    assert_eq!(proxy["detour"], global["outbound"]);
    assert!(server(&cfg, &dns_direct_tag("dns-default"))
        .get("detour")
        .is_none());
    assert_eq!(cfg["dns"]["final"], "dns-default");
    assert!(proxy.get("domain_resolver").is_none());
    // The DNS table is identical in all modes; the core evaluates clash_mode
    // at request time, including after a live PATCH from Direct to Global.
    for mode in [OutboundMode::Direct, OutboundMode::Rule] {
        assert_eq!(config(&p, mode)["dns"], cfg["dns"]);
    }
}

#[test]
fn final_order_preserves_explicit_overrides_then_local_direct_and_fakeip() {
    let mut p = profile();
    p.dns.as_mut().unwrap().fake_ip = Some(true);
    p.dns.as_mut().unwrap().hosts = Some(vec![DnsHostEntry {
        id: "host".into(),
        domain: "host.example".into(),
        address: "192.0.2.1".into(),
        enabled: true,
    }]);
    p.dns.as_mut().unwrap().rules = Some(vec![DnsRule {
        id: "exception".into(),
        kind: "domain".into(),
        value: "exception.example".into(),
        target: "local-dns".into(),
        comment: None,
        enabled: true,
    }]);
    let cfg = config(&p, OutboundMode::Rule);
    let rules = cfg["dns"]["rules"].as_array().unwrap();
    assert_eq!(rules[0]["server"], "local-hosts");
    assert_eq!(rules[1]["domain"], json!(["exception.example"]));
    assert_eq!(rules[2]["domain_suffix"], json!(["local", "home.arpa"]));
    assert_eq!(rules[3]["domain_regex"], json!([r"^[^.]+\.?$"]));
    assert_eq!(rules[4]["clash_mode"], "direct");
    assert_eq!(rules[4]["server"], dns_direct_tag("dns-default"));
    assert_eq!(rules[5]["server"], "fakeip");
    assert_eq!(rules.len(), 6, "no catch-all Local fallback");
}

#[test]
fn endpoint_bootstrap_is_separate_and_missing_proxies_never_fall_back_to_direct() {
    let p = profile();
    let cfg = config(&p, OutboundMode::Global);
    let proxy = cfg["outbounds"]
        .as_array()
        .unwrap()
        .iter()
        .find(|o| o["type"] == "socks")
        .unwrap();
    assert_eq!(proxy["domain_resolver"], "local-dns");
    assert_eq!(proxy["server"], "proxy.example");
    // We retain the domain target capability; only the proxy's own endpoint
    // gets an explicit bootstrap resolver.
    assert!(proxy.get("domain_strategy").is_none());
    let cfg = config(&ZeytunCore::default(), OutboundMode::Global);
    assert_eq!(server(&cfg, "dns-default")["detour"], "block");
}

#[test]
fn editable_final_roundtrips_and_changes_provider_without_a_policy_switch() {
    let dir = std::env::temp_dir().join(format!("dns-final-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let db = Db::new(dir.join("zeytun.db"));
    let mut p = profile();
    p.dns
        .as_mut()
        .unwrap()
        .servers
        .as_mut()
        .unwrap()
        .push(DnsServer {
            tag: "custom".into(),
            name: "Custom".into(),
            address: "https://1.1.1.1/dns-query".into(),
            detour: Some("follow-mode".into()),
        });
    db.save_profile(&p).unwrap();
    let revision = db.config_revision(&p.id).unwrap();
    for mode in [
        OutboundMode::Direct,
        OutboundMode::Global,
        OutboundMode::Rule,
    ] {
        p.outbound_mode = mode;
        db.save_profile(&p).unwrap();
        assert_eq!(db.config_revision(&p.id).unwrap(), revision);
    }
    p.dns.as_mut().unwrap().final_server = Some("custom".into());
    db.save_profile(&p).unwrap();
    assert!(db.config_revision(&p.id).unwrap() > revision);
    let saved = db.load_profile(&p.id).unwrap();
    let raw = ProfileDocument::encode("DNS", None, &saved, &[], ExportMode::Full, vec![]).unwrap();
    assert!(!raw.contains("\"mode\""));
    let imported = ProfileDocument::decode(&raw)
        .unwrap()
        .to_zeytun_core("imported")
        .0;
    assert_eq!(
        imported.dns.as_ref().unwrap().final_server.as_deref(),
        Some("custom")
    );
    let cfg = config(&imported, OutboundMode::Global);
    assert_eq!(cfg["dns"]["final"], "custom");
    assert_eq!(server(&cfg, "custom")["server"], "1.1.1.1");
    assert_eq!(server(&cfg, "custom")["detour"], "root-policy");
    assert!(server(&cfg, &dns_direct_tag("custom"))
        .get("detour")
        .is_none());
    drop(db);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn follow_mode_rules_preserve_priority_and_explicit_routes_in_every_mode() {
    let mut p = profile();
    let dns = p.dns.as_mut().unwrap();
    dns.servers.as_mut().unwrap().push(DnsServer {
        tag: "fixed".into(),
        name: "Fixed".into(),
        address: "9.9.9.9".into(),
        detour: Some("direct".into()),
    });
    dns.rules = Some(vec![
        DnsRule {
            id: "rule".into(),
            kind: "domain_suffix".into(),
            value: "example".into(),
            target: "dns-default".into(),
            comment: None,
            enabled: true,
        },
        DnsRule {
            id: "fixed".into(),
            kind: "domain".into(),
            value: "fixed.test".into(),
            target: "fixed".into(),
            comment: None,
            enabled: true,
        },
    ]);
    let cfg = config(&p, OutboundMode::Global);
    let rules = cfg["dns"]["rules"].as_array().unwrap();
    assert_eq!(rules[0]["domain_suffix"], json!(["example"]));
    assert_eq!(rules[0]["clash_mode"], "direct");
    assert_eq!(rules[0]["server"], dns_direct_tag("dns-default"));
    assert_eq!(rules[1]["server"], "dns-default");
    assert_eq!(rules[2]["server"], "fixed");
    assert!(server(&cfg, "fixed").get("detour").is_none());
    assert!(cfg["dns"]["servers"]
        .as_array()
        .unwrap()
        .iter()
        .all(|server| server["tag"] != "__dns-direct-fixed"));
    for mode in [OutboundMode::Direct, OutboundMode::Rule] {
        assert_eq!(config(&p, mode)["dns"], cfg["dns"]);
    }
}

#[test]
fn local_final_survives_export_import_without_recreating_google() {
    let mut p = profile();
    p.dns.as_mut().unwrap().final_server = Some("local-dns".into());
    let encoded =
        ProfileDocument::encode("local", None, &p, &[], ExportMode::Full, vec![]).unwrap();
    let imported = ProfileDocument::decode(&encoded)
        .unwrap()
        .to_zeytun_core("new")
        .0;
    assert_eq!(
        imported.dns.as_ref().unwrap().final_server.as_deref(),
        Some("local-dns")
    );
    assert_eq!(
        imported
            .dns
            .as_ref()
            .unwrap()
            .servers
            .as_ref()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        config(&imported, OutboundMode::Global)["dns"]["final"],
        "local-dns"
    );
}

#[test]
fn bundled_core_accepts_follow_mode_dns_with_all_modes_fakeip_and_hostname_proxies() {
    let core = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("resources/bin/macos-aarch64/zeytun-core");
    if !core.exists() {
        return;
    }
    let dir = std::env::temp_dir().join(format!("dns-auto-check-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    for fake_ip in [false, true] {
        let mut p = profile();
        p.dns.as_mut().unwrap().fake_ip = Some(fake_ip);
        for mode in [
            OutboundMode::Direct,
            OutboundMode::Global,
            OutboundMode::Rule,
        ] {
            let cfg = config(&p, mode);
            let path = dir.join("config.json");
            std::fs::write(&path, serde_json::to_vec(&cfg).unwrap()).unwrap();
            let output = std::process::Command::new(&core)
                .args(["check", "-c"])
                .arg(path)
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
    }
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn existing_database_preserves_authored_dns_without_extra_migrations() {
    let dir = std::env::temp_dir().join(format!("dns-v1-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("zeytun.db");
    let conn = rusqlite::Connection::open(&path).unwrap();
    conn.execute_batch(include_str!("../migrations/0001_init.sql"))
        .unwrap();
    conn.execute_batch("CREATE TABLE schema_migration (version INTEGER PRIMARY KEY NOT NULL, name TEXT NOT NULL, applied_unix_ms INTEGER NOT NULL);
        INSERT INTO schema_migration VALUES (1, 'init', 0);
        UPDATE profile SET outbound_mode='global' WHERE id='default';
        DELETE FROM dns_config; DELETE FROM dns_server;
        INSERT INTO dns_config (profile_id,final_server,fake_ip) VALUES ('default','custom',0);
        INSERT INTO dns_server (profile_id,tag,address,detour,position,name) VALUES ('default','custom','1.1.1.1','direct',0,'Custom');").unwrap();
    drop(conn);
    let db = Db::new(path.clone());
    let p = db.load_profile("default").unwrap();
    let dns = p.dns.as_ref().unwrap();
    assert_eq!(dns.mode, None);
    assert_eq!(
        dns.servers.as_ref().unwrap()[0].detour.as_deref(),
        Some("direct")
    );
    assert_eq!(dns.final_server.as_deref(), Some("custom"));
    db.save_profile(&p).unwrap();
    drop(db);
    let conn = rusqlite::Connection::open(path).unwrap();
    assert_eq!(
        conn.query_row("SELECT COUNT(*) FROM schema_migration", [], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        1
    );
    drop(conn);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn initial_schema_seeds_editable_dns_and_preserves_an_explicit_local_final() {
    let dir = std::env::temp_dir().join(format!("dns-init-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("zeytun.db");
    let db = Db::new(path.clone());
    let mut p = db.load_profile("default").unwrap();
    let dns = p.dns.as_mut().unwrap();
    assert_eq!(dns.mode, None);
    assert_eq!(dns.final_server.as_deref(), Some("dns-default"));
    let seed = &dns.servers.as_ref().unwrap()[0];
    assert_eq!(seed.address, "https://8.8.8.8/dns-query");
    assert_eq!(seed.detour.as_deref(), Some("follow-mode"));
    let mut meta = db.list_profiles().unwrap().remove(0);
    meta.id = "new-profile".into();
    meta.is_active = false;
    db.create_profile(&meta).unwrap();
    assert_eq!(
        serde_json::to_value(db.load_profile(&meta.id).unwrap().dns).unwrap(),
        serde_json::to_value(&p.dns).unwrap()
    );
    let dns = p.dns.as_mut().unwrap();
    dns.final_server = Some("local-dns".into());
    dns.fake_ip = Some(true);
    dns.rules = Some(vec![DnsRule {
        id: "keep".into(),
        kind: "domain".into(),
        value: "keep.test".into(),
        target: "dns-default".into(),
        comment: Some("Keep".into()),
        enabled: true,
    }]);
    dns.hosts = Some(vec![DnsHostEntry {
        id: "host".into(),
        domain: "host.test".into(),
        address: "192.0.2.1".into(),
        enabled: true,
    }]);
    db.save_profile(&p).unwrap();
    let first = serde_json::to_value(&p.dns).unwrap();
    drop(db);
    let db = Db::new(path.clone());
    assert_eq!(
        serde_json::to_value(db.load_profile("default").unwrap().dns).unwrap(),
        first
    );
    drop(db);
    let conn = rusqlite::Connection::open(path).unwrap();
    let columns = conn
        .prepare("PRAGMA table_info(dns_config)")
        .unwrap()
        .query_map([], |row| row.get::<_, String>(1))
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    assert!(!columns.iter().any(|name| name == "mode"));
    assert_eq!(
        conn.query_row("SELECT COUNT(*) FROM schema_migration", [], |row| row
            .get::<_, i64>(0))
            .unwrap(),
        1
    );
    let persisted: (String, String) = conn.query_row(
        "SELECT final_server, detour FROM dns_config JOIN dns_server USING(profile_id) WHERE profile_id='new-profile'",
        [], |row| Ok((row.get(0)?, row.get(1)?)),
    ).unwrap();
    assert_eq!(persisted, ("dns-default".into(), "follow-mode".into()));
    drop(conn);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn legacy_profile_import_materializes_default_but_current_final_is_authoritative() {
    let p = profile();
    let encoded =
        ProfileDocument::encode("legacy", None, &p, &[], ExportMode::Full, vec![]).unwrap();
    let mut raw: Value = serde_json::from_str(&encoded).unwrap();
    raw["schema_version"] = json!(1);
    raw["profile"]["dns"] = json!({"mode":"automatic","servers":[{"tag":"custom","name":"Custom","address":"9.9.9.9"}],"final_server":"custom"});
    let decoded = ProfileDocument::decode(&raw.to_string())
        .unwrap()
        .to_zeytun_core("legacy")
        .0;
    let dns = decoded.dns.unwrap();
    assert_eq!(dns.final_server.as_deref(), Some("dns-default"));
    assert_eq!(dns.servers.unwrap()[0].tag, "custom");
}

/// Exercises real core routing with isolated loopback TLS DNS and SOCKS exits.
/// No OS DNS changes, TUN, external DNS, or user proxy credentials are involved.
#[test]
#[ignore = "requires localhost sockets, python3, and openssl"]
fn dns_runtime_follows_live_mode_selection_and_fails_closed() {
    let core = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("resources/bin/macos-aarch64/zeytun-core");
    assert!(core.exists());
    let dir = std::env::temp_dir().join(format!("dns-auto-runtime-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let mut p = profile();
    let dns = p.dns.as_mut().unwrap();
    let selected = &mut dns.servers.as_mut().unwrap()[0];
    selected.tag = "chosen-provider".into();
    selected.name = "User chosen DoH".into();
    selected.address = "https://1.1.1.1/dns-query".into();
    dns.final_server = Some(selected.tag.clone());
    dns.rules = Some(vec![DnsRule {
        id: "user-rule".into(),
        kind: "domain_suffix".into(),
        value: "routed.example".into(),
        target: selected.tag.clone(),
        comment: None,
        enabled: true,
    }]);
    let mut fake = p.clone();
    fake.dns.as_mut().unwrap().fake_ip = Some(true);
    let path = dir.join("configs.json");
    std::fs::write(
        &path,
        serde_json::to_vec(&json!({
            "real": config(&p, OutboundMode::Direct),
            "fake": config(&fake, OutboundMode::Direct),
        }))
        .unwrap(),
    )
    .unwrap();
    let output = std::process::Command::new("python3")
        .arg(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests/support/dns_policy_runtime.py"),
        )
        .arg(core)
        .arg(path)
        .output()
        .unwrap();
    std::fs::remove_dir_all(dir).unwrap();
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    println!("{}", String::from_utf8_lossy(&output.stdout));
}
