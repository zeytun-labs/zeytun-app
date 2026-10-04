#[test]
fn external_ip_stun_has_deadline_before_http_fallback() {
    let source = include_str!("../src/lib.rs");
    let command = source
        .split("async fn core_get_external_ip_info(")
        .nth(1)
        .unwrap()
        .split("/// Download the latest GeoIP")
        .next()
        .unwrap();
    assert!(
        command.contains("tokio::time::timeout("),
        "STUN RPC/stream has no deadline"
    );
    assert!(command.contains("fetch_external_ip(active_proxy_port)"));
}
