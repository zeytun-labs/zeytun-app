use super::*;
use crate::core::dto::{Rule, RuleType, TempRule};
use std::{
    io::{Read, Write},
    net::TcpListener,
    sync::atomic::{AtomicBool, Ordering},
    thread::JoinHandle,
    time::Duration,
};

// Exercise real manager commands and HTTP acknowledgements without launching a
// core, acquiring privileges, or changing any system network settings.
struct Fixture {
    manager: std::mem::ManuallyDrop<CoreManager>,
    dir: PathBuf,
    fail_permanent: Arc<AtomicBool>,
    fail_temp: Arc<AtomicBool>,
    stop: Arc<AtomicBool>,
    server: Option<JoinHandle<()>>,
    initial_revision: u64,
}

impl Fixture {
    fn new() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let controller = listener.local_addr().unwrap().to_string();
        listener.set_nonblocking(true).unwrap();
        let stop = Arc::new(AtomicBool::new(false));
        let fail_permanent = Arc::new(AtomicBool::new(false));
        let fail_temp = Arc::new(AtomicBool::new(false));
        let server_stop = stop.clone();
        let server_fail = fail_permanent.clone();
        let server_fail_temp = fail_temp.clone();
        let server = std::thread::spawn(move || {
            'requests: while !server_stop.load(Ordering::Relaxed) {
                let (mut stream, _) = match listener.accept() {
                    Ok(connection) => connection,
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(Duration::from_millis(2));
                        continue;
                    }
                    Err(e) => panic!("mock controller: {e}"),
                };
                stream.set_nonblocking(false).unwrap();
                stream
                    .set_read_timeout(Some(Duration::from_secs(2)))
                    .unwrap();
                let mut request = Vec::new();
                let mut buffer = [0; 4096];
                let header_end = loop {
                    let count = match stream.read(&mut buffer) {
                        Ok(0) | Err(_) => continue 'requests,
                        Ok(count) => count,
                    };
                    request.extend_from_slice(&buffer[..count]);
                    if let Some(pos) = request.windows(4).position(|w| w == b"\r\n\r\n") {
                        break pos + 4;
                    }
                };
                let headers = String::from_utf8_lossy(&request[..header_end]).to_string();
                let length = headers
                    .lines()
                    .find_map(|line| {
                        let (key, value) = line.split_once(':')?;
                        key.eq_ignore_ascii_case("content-length")
                            .then(|| value.trim().parse::<usize>().unwrap())
                    })
                    .unwrap_or(0);
                while request.len() < header_end + length {
                    let count = match stream.read(&mut buffer) {
                        Ok(0) | Err(_) => continue 'requests,
                        Ok(count) => count,
                    };
                    request.extend_from_slice(&buffer[..count]);
                }
                let status = if (headers.starts_with("PUT /permanent-rules/")
                    && server_fail.load(Ordering::Relaxed))
                    || (headers.starts_with("PUT /temp-rules/")
                        && server_fail_temp.load(Ordering::Relaxed))
                {
                    "500 Internal Server Error"
                } else {
                    "204 No Content"
                };
                let _ = write!(
                    stream,
                    "HTTP/1.1 {status}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                );
            }
        });
        let dir = std::env::temp_dir().join(format!("live-apply-{}", uuid::Uuid::new_v4()));
        let mut manager =
            CoreManager::new(dir.clone(), None, Arc::new(EventBus::headless())).unwrap();
        manager.profile.rules.insert(0, rule(1));
        let mut disabled = rule(2);
        disabled.enabled = false;
        manager.profile.rules.insert(1, disabled);
        manager.profile.temp_rules.push(temp_rule(3));
        manager.db.save_profile(&manager.profile).unwrap();
        let initial_revision = manager.saved_revision(&manager.profile.id).unwrap();
        manager.lifecycle.assume_running_for_test(
            &manager.profile,
            initial_revision,
            ClashApiClient::new(controller, None),
        );
        Self {
            manager: std::mem::ManuallyDrop::new(manager),
            dir,
            fail_permanent,
            fail_temp,
            stop,
            server: Some(server),
            initial_revision,
        }
    }

    fn assert_applied(&mut self) {
        let status = self.manager.status();
        assert!(
            !status.pending_apply,
            "unexpected pending revision: {status:?}"
        );
        assert_eq!(status.applied_revision, Some(status.saved_revision));
    }

    fn assert_pending(&mut self) {
        let status = self.manager.status();
        assert!(status.pending_apply);
        assert_eq!(status.applied_revision, Some(self.initial_revision));
        assert!(status.saved_revision > self.initial_revision);
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        let joined = self.server.take().unwrap().join();
        unsafe {
            std::mem::ManuallyDrop::drop(&mut self.manager);
        }
        let removed = (|| {
            for _ in 0..10 {
                match std::fs::remove_dir_all(&self.dir) {
                    Ok(()) => return Ok(()),
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
                    Err(_) => std::thread::sleep(Duration::from_millis(20)),
                }
            }
            std::fs::remove_dir_all(&self.dir)
        })();
        if !std::thread::panicking() {
            joined.unwrap();
            removed.unwrap();
        }
    }
}

fn rule(id: u64) -> Rule {
    Rule {
        id,
        kind: RuleType::Domain,
        value: format!("{id}.example.test"),
        outbound: "direct".into(),
        comment: String::new(),
        rule_set: None,
        orphaned: false,
        enabled: true,
    }
}

fn temp_rule(id: u64) -> TempRule {
    TempRule {
        id,
        kind: RuleType::Domain,
        value: format!("{id}.example.test"),
        outbound: "direct".into(),
        comment: String::new(),
        rule_set: None,
        orphaned: false,
        enabled: true,
        expires_at: i64::MAX,
        session: false,
        ask_group_key: None,
    }
}

#[test]
fn permanent_rule_actions_acknowledge_the_saved_revision() {
    for action in ["disable", "enable", "delete", "add", "reorder"] {
        let mut f = Fixture::new();
        let mut rules = f.manager.profile.rules.clone();
        match action {
            "disable" => rules[0].enabled = false,
            "enable" => rules[1].enabled = true,
            "delete" => {
                rules.remove(0);
            }
            "add" => rules.insert(0, rule(4)),
            "reorder" => {
                rules.insert(0, rule(4));
                rules.swap(0, 1);
            }
            _ => unreachable!(),
        }
        f.manager.update_rules(rules).unwrap();
        f.assert_applied();
    }
}

#[test]
fn temp_session_promotion_and_ask_paths_acknowledge_live_changes() {
    for action in [
        "temp-disable",
        "temp-delete",
        "temp-add",
        "session",
        "promote",
        "ask",
    ] {
        let mut f = Fixture::new();
        match action {
            "temp-disable" => {
                let mut rules = f.manager.profile.temp_rules.clone();
                rules[0].enabled = false;
                f.manager.update_temp_rules(rules).unwrap();
            }
            "temp-delete" => f.manager.update_temp_rules(vec![]).unwrap(),
            "temp-add" => f.manager.update_temp_rules(vec![temp_rule(4)]).unwrap(),
            "session" => {
                let mut rules = f.manager.profile.temp_rules.clone();
                let mut session = temp_rule(4);
                session.session = true;
                session.expires_at = 0;
                rules.push(session);
                f.manager.update_temp_rules(rules).unwrap();
            }
            "promote" => f.manager.promote_temp_rule(3).unwrap(),
            "ask" => f
                .manager
                .decide_connection_ask(
                    "ask-id".into(),
                    "direct".into(),
                    false,
                    true,
                    Some("/Applications/Example.app/Contents/MacOS/Example".into()),
                    None,
                    None,
                    None,
                )
                .unwrap(),
            _ => unreachable!(),
        }
        f.assert_applied();
    }
}

#[test]
fn live_rules_preserve_unapplied_settings_and_graph_changes() {
    for pending in ["port", "listen", "log", "dns", "proxy", "final", "ask"] {
        let mut f = Fixture::new();
        match pending {
            "port" => f.manager.set_listen_port(7070).unwrap(),
            "listen" => f.manager.set_allow_lan(true).unwrap(),
            "log" => f.manager.set_log_level("debug".into()).unwrap(),
            "dns" => f
                .manager
                .update_dns_hosts(vec![serde_json::from_value(
                        serde_json::json!({"id":"host", "domain":"example.test", "address":"192.0.2.1", "enabled":true}),
                )
                .unwrap()])
                .unwrap(),
            "proxy" => {
                f.manager
                    .proxy_import_link("socks5://node.test:1080#Node")
                    .unwrap();
            }
            "ask" => f.manager.set_connection_ask(serde_json::from_value(
                serde_json::json!({"enabled":true})
            ).unwrap()).unwrap(),
            "final" => {
                f.manager
                    .profile
                    .rules
                    .iter_mut()
                    .find(|r| r.kind == RuleType::Final)
                    .unwrap()
                    .outbound = DEFAULT_SELECTOR_POLICY_TAG.into()
            }
            _ => unreachable!(),
        }
        let mut rules = f.manager.profile.rules.clone();
        rules[0].enabled = false;
        f.manager.update_rules(rules).unwrap();
        f.assert_pending();
    }
}

#[test]
fn rejected_overlay_and_partial_promotion_remain_pending() {
    for promote in [false, true] {
        let mut f = Fixture::new();
        f.fail_permanent.store(true, Ordering::Relaxed);
        let result = if promote {
            f.manager.promote_temp_rule(3)
        } else {
            let mut rules = f.manager.profile.rules.clone();
            rules[0].enabled = false;
            f.manager.update_rules(rules)
        };
        assert!(result.is_err());
        assert!(f.manager.lifecycle.applied_profile().unwrap().rules[0].enabled);
        f.assert_pending();
    }
}

#[test]
fn successful_retry_clears_pending_after_a_rejected_overlay() {
    let mut f = Fixture::new();
    let mut rules = f.manager.profile.rules.clone();
    rules[0].enabled = false;
    f.fail_permanent.store(true, Ordering::Relaxed);
    assert!(f.manager.update_rules(rules.clone()).is_err());
    f.fail_permanent.store(false, Ordering::Relaxed);
    f.manager.update_rules(rules).unwrap();
    f.assert_applied();
}

#[test]
fn rejected_temp_overlay_remains_pending_until_retry() {
    let mut f = Fixture::new();
    let mut rules = f.manager.profile.temp_rules.clone();
    rules[0].enabled = false;
    f.fail_temp.store(true, Ordering::Relaxed);
    assert!(f.manager.update_temp_rules(rules.clone()).is_err());
    assert!(f.manager.lifecycle.applied_profile().unwrap().temp_rules[0].enabled);
    f.fail_temp.store(false, Ordering::Relaxed);
    f.manager.update_temp_rules(rules).unwrap();
    f.assert_applied();
}

#[test]
fn pruning_expired_temp_rules_acknowledges_the_live_removal() {
    let mut f = Fixture::new();
    f.manager.profile.temp_rules[0].expires_at = 1;
    f.manager.db.save_profile(&f.manager.profile).unwrap();
    assert!(f.manager.prune_expired_temp_rules().unwrap());
    assert!(f
        .manager
        .lifecycle
        .applied_profile()
        .unwrap()
        .temp_rules
        .is_empty());
    f.assert_applied();
}

#[test]
fn overlay_does_not_acknowledge_a_newer_saved_snapshot() {
    let mut f = Fixture::new();
    f.manager.profile.rules[0].enabled = false;
    f.manager.db.save_profile(&f.manager.profile).unwrap();
    let mut newer = f.manager.profile.clone();
    newer.local_proxy.log_level = Some("debug".into());
    f.manager.db.save_local_proxy(&newer.local_proxy).unwrap();
    // Another writer saved a change after the command's in-memory snapshot.
    f.manager.push_permanent_rules_overlay().unwrap();
    f.assert_pending();
}

#[test]
fn live_temp_rules_and_mode_switches_preserve_real_pending_changes() {
    for pending in [false, true] {
        let mut f = Fixture::new();
        if pending {
            f.manager.set_listen_port(7070).unwrap();
        }
        f.manager.set_mode(OutboundMode::Direct).unwrap();
        let mut rules = f.manager.profile.temp_rules.clone();
        rules[0].enabled = false;
        f.manager.update_temp_rules(rules).unwrap();
        f.manager.set_mode(OutboundMode::Rule).unwrap();
        if pending {
            f.assert_pending();
        } else {
            f.assert_applied();
        }
    }
}

#[test]
fn unchanged_subscription_refresh_preserves_applied_and_pending_state() {
    for pending in [false, true] {
        let mut f = Fixture::new();
        f.manager
            .subscription_create(
                "default",
                crate::core::dto::SubscriptionSettings {
                    name: "Source".into(),
                    url: "https://example.test/sub".into(),
                    enabled: true,
                    auto_update: false,
                    update_interval_hours: 24,
                },
            )
            .unwrap();
        f.manager
            .profile_apply_refresh(
                "default",
                "socks5://node.test:1080#Node",
                crate::core::subscription::SubHeaders::default(),
                false,
            )
            .unwrap();
        let applied = f.manager.profile.clone();
        f.initial_revision = f.manager.saved_revision("default").unwrap();
        let clash = f.manager.lifecycle.clash_api().unwrap().clone();
        f.manager
            .lifecycle
            .assume_running_for_test(&applied, f.initial_revision, clash);
        if pending {
            f.manager.set_listen_port(7070).unwrap();
        }

        let summary = f
            .manager
            .profile_apply_refresh(
                "default",
                "socks5://node.test:1080#Renamed",
                crate::core::subscription::SubHeaders::default(),
                false,
            )
            .unwrap();
        assert_eq!(summary.proxies_updated, 0);
        assert_eq!(summary.proxies_added, 0);
        assert_eq!(summary.proxies_removed, 0);
        assert!(
            f.manager.lifecycle.is_running(),
            "refresh must not restart the core"
        );
        if pending {
            f.assert_pending();
        } else {
            f.assert_applied();
        }
    }
}

#[test]
fn stopped_or_different_applied_profile_is_not_acknowledged() {
    for stopped in [false, true] {
        let mut f = Fixture::new();
        if stopped {
            // status notices the absent owned child; no real core was started.
            f.manager.status();
        } else {
            let mut other = f.manager.profile.clone();
            other.id = "other".into();
            let clash = f.manager.lifecycle.clash_api().unwrap().clone();
            f.manager
                .lifecycle
                .assume_running_for_test(&other, f.initial_revision, clash);
        }
        let mut rules = f.manager.profile.rules.clone();
        rules[0].enabled = false;
        f.manager.update_rules(rules).unwrap();
        f.assert_pending();
    }
}
