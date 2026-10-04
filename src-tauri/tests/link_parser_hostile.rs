//! Round-trip and hostile-input tests for `link_parser`.
//!
//! Parsing runs on user-pasted subscription bodies and clipboard links, so the
//! parser must reject garbage with an `Err` — never panic — and a
//! parsed→re-encoded link must keep the fields the app acts on.

use zeytun_lib::core::link_parser::{proxy_from_link, proxy_to_link};
use zeytun_lib::core::models::proxy::Protocol;
use zeytun_lib::core::{dto::ProxyOrigin, models::proxy::ProxyServer};

fn parse(link: &str) -> Result<ProxyServer, String> {
    let proxy = proxy_from_link(link, ProxyOrigin::Manual, "t".into())?;
    proxy_to_proxy_server(&proxy)
}

use zeytun_lib::core::link_parser::proxy_to_proxy_server;

#[test]
fn vless_standard_link_roundtrips() {
    let link = "vless://5e3daa0d-9c9a-4f0f-9a1d-6ce35eeb3fc3@example.com:443?security=tls&sni=a.example&type=ws&path=%2Fws#My%20Node";
    let server = parse(link).expect("parse vless");
    assert!(matches!(server.protocol, Protocol::Vless(_)));
    assert_eq!(server.name, "My Node");
    assert_eq!(server.address, "example.com");
    assert_eq!(server.port, 443);

    // Round-trip: re-encoding the parsed server must yield a link that parses
    // back to the same server/port/uuid.
    let out = proxy_to_link(&proxy_from_link(link, ProxyOrigin::Manual, "t".into()).unwrap())
        .expect("encode");
    let reparsed = parse(&out).expect("re-parse encoded link");
    assert_eq!(reparsed.address, "example.com");
    assert_eq!(reparsed.port, 443);
    if let Protocol::Vless(v) = reparsed.protocol {
        assert_eq!(v.uuid, "5e3daa0d-9c9a-4f0f-9a1d-6ce35eeb3fc3");
    } else {
        panic!("protocol changed across round-trip");
    }
}

#[test]
fn ss_2022_link_roundtrips() {
    // 32-byte base64url password (ss2022 blake3-aes-128-gcm).
    let link =
        "ss://2022-blake3-aes-128-gcm:ZGVmZ29ob3N0MjJibGFrZTNhZXNpdW06dGVzdA@example.com:8388#ss22";
    let server = parse(link).expect("parse ss2022");
    assert!(matches!(server.protocol, Protocol::Shadowsocks(_)));
    let out = proxy_to_link(&proxy_from_link(link, ProxyOrigin::Manual, "t".into()).unwrap())
        .expect("encode");
    assert!(out.starts_with("ss://"), "encoded link keeps scheme: {out}");
}

#[test]
fn hostile_inputs_err_never_panic() {
    let cases: &[&str] = &[
        "",
        "   ",
        "not-a-link",
        "vless://",
        "vless://@@@",
        "vless://:0@",
        "ss://",
        "ss://::::",
        "vmess://",
        "vmess://%%%zz",
        "vmess://eyJub3RfanNvbiI6", // invalid base64 payload JSON
        "trojan://@:/path",
        "hysteria2://",
        "hysteria2://pass@[:::1]:99999",
        "http://user:pass@[999.999.999.999]:notaport",
        "socks://a b c",
        "vless://uuid@host:99999", // port overflow
        "vless://uuid@host:-1",    // negative port
        "%00%01%02%03",
    ];
    for case in cases {
        let result = std::panic::catch_unwind(|| parse(case));
        match result {
            Ok(Ok(_)) => panic!("hostile input parsed as a proxy: {case:?}"),
            Ok(Err(_)) => {} // rejected — correct
            Err(_) => panic!("parser PANICKED on: {case:?}"),
        }
    }
}

#[test]
fn long_input_is_bounded() {
    // 1 MiB of garbage must come back as an error quickly, not OOM/hang.
    let big = "vless://".to_string() + &"A".repeat(1024 * 1024);
    let result = std::panic::catch_unwind(|| parse(&big));
    assert!(
        matches!(result, Ok(Err(_))),
        "1MiB garbage must be rejected"
    );
}
