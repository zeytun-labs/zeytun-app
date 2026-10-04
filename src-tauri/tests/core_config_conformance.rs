//! Protocol conformance: every proxy protocol the app emits must be accepted by
//! the bundled core, and every emitted share link must round-trip.
//!
//! Both tests go through the REAL parse path (`proxy_from_link` → `zeytun-config`
//! → `proxy_to_proxy_server` → `OutboundFactory`), so a regression in the link
//! parser, the compiler, or the core's own validation all fail here.
//!
//! Skipped when `resources/bin/*/zeytun-core` is absent so a fresh checkout
//! without assets still passes.

use std::path::PathBuf;

use zeytun_lib::core::compiler::outbound_factory::OutboundFactory;
use zeytun_lib::core::dto::ProxyOrigin;
use zeytun_lib::core::link_parser::{proxy_from_link, proxy_to_link, proxy_to_proxy_server};
use zeytun_lib::core::models::zeytun_core::outbound::{DirectOutbound, Outbound, OutboundType};

/// Public key from `zeytun-core generate reality-keypair` — the core rejects a
/// `reality.public_key` that is not valid base64 of the right length.
const REALITY_PUBLIC_KEY: &str = "uxeS0xg7gl6yz-lWx8HB-w5fH4_CwzxWaxwFejKfIyA";

/// base64 of 32 zero-prefixed bytes; `2022-blake3-aes-256-gcm` needs a 32-byte
/// key, and the core rejects anything that is not valid base64 of that length.
const SS_2022_PASSWORD: &str = "AAECAwQFBgcICQoLDA0ODxAREhMUFRYXGBkaGxwdHh8=";

fn ss_link() -> String {
    let userinfo = format!("2022-blake3-aes-256-gcm:{SS_2022_PASSWORD}");
    // base64 with padding stripped, the SIP002 wire form `parse_shadowsocks`
    // decodes via `decode_base64_to_string` (padding-tolerant).
    let b64 = base64_std(&userinfo).trim_end_matches('=').to_string();
    format!("ss://{b64}@ss.example.com:8388#ss-2022")
}

fn vmess_link() -> String {
    let payload = serde_json::json!({
        "v": "2",
        "ps": "vmess-node",
        "add": "vmess.example.com",
        "port": "443",
        "id": "2e4f6a8c-0b1d-4e3f-9a5c-7d8e9f0a1b2c",
        "aid": "0",
        "scy": "auto",
        "net": "tcp",
        "type": "none",
        "tls": "tls",
        "sni": "vmess.example.com"
    });
    format!(
        "vmess://{}",
        base64_std(&serde_json::to_string(&payload).unwrap())
    )
}

/// Standard-alphabet base64 via the `base64` crate already in Cargo.toml.
fn base64_std(input: &str) -> String {
    use base64::Engine as _;
    base64::engine::general_purpose::STANDARD.encode(input)
}

/// One share link per protocol, in the exact forms `zeytun-config` accepts.
///
/// Core strictness satisfied here so the test asserts OUR compiler, not a broken
/// fixture: reality `pbk` is a real generated key, the ss password is valid
/// base64 of 32 bytes, and hysteria2/tuic links carry TLS parameters (the parser
/// always enables TLS for both, which is what the core requires).
fn fixture_links() -> Vec<(&'static str, String)> {
    vec![
        (
            "vless",
            format!(
                "vless://1a2b3c4d-5e6f-4a7b-8c9d-0e1f2a3b4c5d@vless.example.com:443\
                 ?encryption=none&type=tcp&security=reality&sni=www.microsoft.com\
                 &fp=chrome&pbk={REALITY_PUBLIC_KEY}&sid=a1b2c3d4\
                 &flow=xtls-rprx-vision#vless-reality"
            ),
        ),
        ("vmess", vmess_link()),
        (
            "trojan",
            "trojan://trojanpass@trojan.example.com:443?type=ws&path=%2Ftrojan\
             &security=tls&sni=trojan.example.com#trojan-ws"
                .to_string(),
        ),
        ("shadowsocks", ss_link()),
        (
            "hysteria2",
            "hysteria2://hy2pass@hy2.example.com:443?sni=hy2.example.com&insecure=1#hy2-node"
                .to_string(),
        ),
        (
            "tuic",
            "tuic://3f1a2b4c-5d6e-4f70-8192-a3b4c5d6e7f8:tuicpass@tuic.example.com:443\
             ?sni=tuic.example.com&congestion_control=bbr#tuic-node"
                .to_string(),
        ),
        (
            "socks",
            "socks5://user:***@socks.example.com:1080#socks5-node".to_string(),
        ),
        (
            "http",
            "http://user:***@http.example.com:8080#http-node".to_string(),
        ),
    ]
}

fn core_binary() -> Option<PathBuf> {
    let base = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/bin");
    for arch in ["macos-aarch64", "macos-x86_64"] {
        let candidate = base.join(arch).join("zeytun-core");
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

fn temp_dir(label: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "zeytun-conformance-{label}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Build one server per protocol through the real parse path.
fn build_servers() -> Vec<(String, String, Outbound)> {
    let factory = OutboundFactory;
    fixture_links()
        .into_iter()
        .map(|(name, link)| {
            let tag = format!("{name}-node");
            let proxy = proxy_from_link(&link, ProxyOrigin::Manual, tag.clone())
                .unwrap_or_else(|e| panic!("{name}: proxy_from_link failed: {e}\nlink: {link}"));
            let server = proxy_to_proxy_server(&proxy)
                .unwrap_or_else(|e| panic!("{name}: proxy_to_proxy_server failed: {e}"));
            let outbound = factory
                .build_proxy_outbound(&server, tag.clone())
                .unwrap_or_else(|e| panic!("{name}: build_proxy_outbound failed: {e}"));
            (name.to_string(), tag, outbound)
        })
        .collect()
}

/// Compile one config holding every protocol outbound, then run the core's own
/// `check` on it. Any protocol the core rejects fails with its exact stderr.
#[test]
fn core_accepts_every_protocol_outbound() {
    let Some(core) = core_binary() else {
        println!(
            "SKIP: zeytun-core binary not found under resources/bin/ — no assets in this checkout"
        );
        return;
    };

    let built = build_servers();
    println!("built {} outbounds:", built.len());
    for (name, tag, outbound) in &built {
        let kind = serde_json::to_value(outbound)
            .ok()
            .and_then(|v| v.get("type").and_then(|t| t.as_str()).map(str::to_string))
            .unwrap_or_else(|| "?".into());
        println!("  {name:12} tag={tag:16} type={kind}");
    }

    let mut outbounds: Vec<serde_json::Value> = vec![
        serde_json::to_value(Outbound {
            tag: "direct".into(),
            outbound_type: OutboundType::Direct(DirectOutbound::default()),
        })
        .unwrap(),
        serde_json::to_value(Outbound {
            tag: "block".into(),
            outbound_type: OutboundType::Block,
        })
        .unwrap(),
    ];
    outbounds.extend(
        built
            .iter()
            .map(|(_, _, o)| serde_json::to_value(o).expect("outbound must serialize")),
    );

    let config = serde_json::json!({
        "log": { "disabled": false, "level": "info", "output": "stdout", "timestamp": true },
        "inbounds": [{
            "tag": "mixed-in",
            "type": "mixed",
            "listen": "127.0.0.1",
            "listen_port": 6099
        }],
        "outbounds": outbounds,
        "route": { "final": "direct" }
    });

    let dir = temp_dir("check");
    let path = dir.join("config.json");
    std::fs::write(&path, serde_json::to_vec_pretty(&config).unwrap()).unwrap();
    println!("config: {}", path.display());

    let output = std::process::Command::new(&core)
        .arg("check")
        .arg("-c")
        .arg(&path)
        .output()
        .unwrap_or_else(|e| panic!("failed to run {}: {e}", core.display()));

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    println!("core check exit={:?}", output.status.code());
    if !stdout.trim().is_empty() {
        println!("--- stdout ---\n{}", stdout.trim());
    }
    if !stderr.trim().is_empty() {
        println!("--- stderr ---\n{}", stderr.trim());
    }

    assert!(
        output.status.success(),
        "zeytun-core rejected the compiled config ({} outbounds: {})\nstderr:\n{}\nstdout:\n{}\nconfig: {}",
        built.len(),
        built
            .iter()
            .map(|(n, _, _)| n.as_str())
            .collect::<Vec<_>>()
            .join(", "),
        stderr.trim(),
        stdout.trim(),
        path.display()
    );

    std::fs::remove_dir_all(&dir).ok();
}

/// `proxy_to_link` must produce a link the parser reads back as the same
/// protocol at the same address and port.
#[test]
fn share_links_round_trip_through_the_parser() {
    let mut checked = 0usize;
    for (name, link) in fixture_links() {
        let tag = format!("{name}-rt");
        let proxy = proxy_from_link(&link, ProxyOrigin::Manual, tag.clone())
            .unwrap_or_else(|e| panic!("{name}: proxy_from_link failed: {e}"));
        let server = proxy_to_proxy_server(&proxy)
            .unwrap_or_else(|e| panic!("{name}: proxy_to_proxy_server failed: {e}"));

        let emitted =
            proxy_to_link(&proxy).unwrap_or_else(|e| panic!("{name}: proxy_to_link failed: {e}"));
        println!("{name:12} {emitted}");

        let again = proxy_from_link(&emitted, ProxyOrigin::Manual, tag.clone())
            .unwrap_or_else(|e| panic!("{name}: re-parse of `{emitted}` failed: {e}"));
        let server_again = proxy_to_proxy_server(&again)
            .unwrap_or_else(|e| panic!("{name}: re-convert failed: {e}"));

        assert_eq!(
            again.protocol, proxy.protocol,
            "{name}: protocol name changed on round-trip"
        );
        assert_eq!(
            server_again.protocol.name(),
            server.protocol.name(),
            "{name}: protocol changed on round-trip"
        );
        assert_eq!(
            server_again.address, server.address,
            "{name}: address changed on round-trip"
        );
        assert_eq!(
            server_again.port, server.port,
            "{name}: port changed on round-trip"
        );
        checked += 1;
    }
    assert_eq!(checked, 8, "expected one round-trip per protocol");
}
