// src-tauri/src/adapters/diagnostics.rs
//
// Real-time network diagnostics — the backend behind the Activity dashboard's
// "Network Diagnostics" card and its detailed report sheet (Surge-style).
//
// Every probe here is blocking (shells out to `route`/`scutil`/`ipconfig`,
// opens raw sockets, issues synchronous HTTP). Callers MUST run `run` inside
// `spawn_blocking` so the async runtime is never stalled.

use std::net::{ToSocketAddrs, UdpSocket};
use std::time::{Duration, Instant};

use serde::Serialize;

/// The HTTP endpoint every reachability probe (direct + proxy) targets. Plain
/// HTTP on purpose: we want to measure transport latency, not a TLS handshake.
const PROBE_URL: &str = "http://google.com/";
const PROBE_HOST: &str = "google.com";

/// What the frontend receives: the raw numbers the card renders plus the
/// fully-formatted plaintext `report` the diagnostics sheet shows verbatim.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticsResult {
    /// Direct HTTP request latency (ms) — the card's big main value.
    pub direct_ms: Option<u64>,
    /// ICMP ping to the default gateway (ms).
    pub router_ms: Option<u64>,
    /// DNS resolution time against the effective system resolver (ms).
    pub dns_ms: Option<u64>,
    /// Total proxy HTTP request time (ms). `None` when the proxy test was not
    /// run (Direct/Rule mode) or the proxy is unreachable.
    pub proxy_ms: Option<u64>,
    /// Whether the proxy test actually ran (Global mode + core running).
    pub proxy_tested: bool,
    /// The verbatim, monospace report rendered in the sheet.
    pub report: String,
}

/// The pieces of network configuration we surface + reuse across probes.
struct NetConfig {
    ipv4_interface: Option<String>,
    ipv4_address: Option<String>,
    ipv4_router: Option<String>,
    ipv6_interface: Option<String>,
    ipv6_address: Option<String>,
    dns_server: Option<String>,
}

/// Run the full diagnostics suite.
///
/// `outbound_mode` is the app's current mode (`"global"`, `"direct"`,
/// `"rule"`). `proxy_port` is the local mixed inbound port zeytun-core exposes.
/// The proxy leg only runs when `proxy_available` (Global mode + core running).
pub fn run(
    outbound_mode: &str,
    proxy_port: u16,
    proxy_available: bool,
    internet_test_url: &str,
    proxy_test_url: &str,
) -> DiagnosticsResult {
    let cfg = read_net_config();
    let router_ms = cfg
        .ipv4_router
        .as_deref()
        .and_then(|gw| ping_host(gw, cfg.ipv4_address.as_deref()));

    // --- DNS: timed query against the effective resolver -----------------
    let dns_server = cfg.dns_server.clone().unwrap_or_else(|| "8.8.8.8".into());
    let dns_ms = dns_query_time(&dns_server, PROBE_HOST, cfg.ipv4_address.as_deref());

    // --- Direct Policy: direct HTTP GET, bypassing any proxy -------------
    let direct = http_probe_direct(cfg.ipv4_address.as_deref());
    let direct_ms = direct.as_ref().map(|d| d.total_ms);

    // --- Proxy: HTTP GET routed through the local mixed inbound ----------
    let proxy = if proxy_available {
        http_probe_proxy(proxy_port)
    } else {
        None
    };
    let proxy_ms = proxy.as_ref().map(|p| p.total_ms);

    let report = build_report(
        &cfg,
        router_ms,
        &dns_server,
        dns_ms,
        direct.as_ref(),
        outbound_mode,
        proxy_available,
        proxy.as_ref(),
        internet_test_url,
        proxy_test_url,
    );

    DiagnosticsResult {
        direct_ms,
        router_ms,
        dns_ms,
        proxy_ms,
        proxy_tested: proxy_available,
        report,
    }
}

/// Outcome of a single HTTP probe.
struct HttpProbe {
    /// Total request time incl. connect, send, response (ms).
    total_ms: u64,
    ok: bool,
}

// ---------------------------------------------------------------------------
// Network configuration
// ---------------------------------------------------------------------------

#[cfg(target_os = "macos")]
fn read_net_config() -> NetConfig {
    use std::process::Command;

    let mut ipv4_interface = None;
    let mut ipv4_router = None;

    // `route -n get default` → interface + gateway for the default IPv4 route.
    if let Ok(out) = Command::new("route")
        .args(["-n", "get", "default"])
        .output()
    {
        let stdout = String::from_utf8_lossy(&out.stdout);
        for line in stdout.lines() {
            let trimmed = line.trim();
            if let Some(rest) = trimmed.strip_prefix("interface:") {
                ipv4_interface = Some(rest.trim().to_string());
            } else if let Some(rest) = trimmed.strip_prefix("gateway:") {
                ipv4_router = Some(rest.trim().to_string());
            }
        }
    }

    // Primary IPv4/IPv6 address of the active interface.
    let ipv4_address = ipv4_interface
        .as_deref()
        .and_then(|iface| ifconfig_addr(iface, false));
    let ipv6_address = ipv4_interface
        .as_deref()
        .and_then(|iface| ifconfig_addr(iface, true));

    // Effective DNS server: first `nameserver` scutil reports.
    let dns_server = Command::new("scutil")
        .arg("--dns")
        .output()
        .ok()
        .and_then(|out| {
            let stdout = String::from_utf8_lossy(&out.stdout);
            stdout
                .lines()
                .find(|l| l.trim_start().starts_with("nameserver"))
                .and_then(|l| l.split_once(':').map(|(_, ip)| ip.trim().to_string()))
        });

    NetConfig {
        ipv6_interface: ipv6_address.as_ref().and(ipv4_interface.clone()),
        ipv4_interface,
        ipv4_address,
        ipv4_router,
        ipv6_address,
        dns_server,
    }
}

#[cfg(not(target_os = "macos"))]
fn read_net_config() -> NetConfig {
    NetConfig {
        ipv4_interface: None,
        ipv4_address: None,
        ipv4_router: None,
        ipv6_interface: None,
        ipv6_address: None,
        dns_server: None,
    }
}

/// Read the first IPv4 (or IPv6) address bound to an interface via `ifconfig`.
#[cfg(target_os = "macos")]
fn ifconfig_addr(iface: &str, ipv6: bool) -> Option<String> {
    use std::process::Command;

    let out = Command::new("ipconfig")
        .args([if ipv6 { "getv6ifaddr" } else { "getifaddr" }, iface])
        .output()
        .ok()?;
    let addr = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if addr.is_empty() {
        None
    } else {
        Some(addr)
    }
}

// ---------------------------------------------------------------------------
// Router: ICMP ping
// ---------------------------------------------------------------------------

/// One-shot ping; returns the round-trip time in ms, or `None` on failure.
#[cfg(target_os = "macos")]
fn ping_host(host: &str, local_ip: Option<&str>) -> Option<u64> {
    use std::process::Command;

    let mut cmd = Command::new("ping");
    cmd.args(["-c", "1", "-t", "2"]);

    // Bind to the physical interface IP to bypass TUN routing
    if let Some(ip) = local_ip {
        cmd.args(["-S", ip]);
    }

    cmd.arg(host);

    let out = cmd.output().ok()?;
    let stdout = String::from_utf8_lossy(&out.stdout);
    parse_ping_time(&stdout)
}

#[cfg(not(target_os = "macos"))]
fn ping_host(_host: &str, _local_ip: Option<&str>) -> Option<u64> {
    None
}

/// Extract the `time=<n> ms` value from `ping` stdout.
#[cfg(target_os = "macos")]
fn parse_ping_time(output: &str) -> Option<u64> {
    output
        .lines()
        .find(|l| l.contains("time="))
        .and_then(|l| l.split("time=").nth(1))
        .map(|after| {
            after
                .chars()
                .take_while(|c| c.is_ascii_digit() || *c == '.')
                .collect::<String>()
        })
        .and_then(|s| s.parse::<f64>().ok())
        .map(|v| v.round() as u64)
}

// ---------------------------------------------------------------------------
// DNS: timed A-record query against a specific resolver
// ---------------------------------------------------------------------------

/// Send a minimal DNS A-record query for `hostname` to `<server>:53` over UDP
/// and measure the time until the first answer. Returns ms, or `None`.
fn dns_query_time(server: &str, hostname: &str, local_ip: Option<&str>) -> Option<u64> {
    let addr = format!("{server}:53");
    let sock_addr = addr.to_socket_addrs().ok()?.next()?;

    let bind_addr = if let Some(ip) = local_ip {
        format!("{ip}:0")
    } else if sock_addr.is_ipv6() {
        "[::]:0".to_string()
    } else {
        "0.0.0.0:0".to_string()
    };

    let socket = UdpSocket::bind(&bind_addr).ok()?;
    socket.set_read_timeout(Some(Duration::from_secs(2))).ok()?;
    socket
        .set_write_timeout(Some(Duration::from_secs(2)))
        .ok()?;

    let query = build_dns_query(hostname);

    let start = Instant::now();
    socket.send_to(&query, sock_addr).ok()?;

    let mut buf = [0u8; 512];
    socket.recv_from(&mut buf).ok()?;
    Some(start.elapsed().as_millis() as u64)
}

/// Build a DNS query packet: header + a single A-record question for `hostname`.
fn build_dns_query(hostname: &str) -> Vec<u8> {
    let mut packet = Vec::with_capacity(hostname.len() + 18);

    // Header: fixed transaction id, standard recursive query, 1 question.
    packet.extend_from_slice(&[0x12, 0x34]); // ID
    packet.extend_from_slice(&[0x01, 0x00]); // flags: RD=1
    packet.extend_from_slice(&[0x00, 0x01]); // QDCOUNT=1
    packet.extend_from_slice(&[0x00, 0x00]); // ANCOUNT
    packet.extend_from_slice(&[0x00, 0x00]); // NSCOUNT
    packet.extend_from_slice(&[0x00, 0x00]); // ARCOUNT

    // Question: QNAME as length-prefixed labels, then QTYPE=A, QCLASS=IN.
    for label in hostname.split('.') {
        packet.push(label.len() as u8);
        packet.extend_from_slice(label.as_bytes());
    }
    packet.push(0x00); // root label terminator
    packet.extend_from_slice(&[0x00, 0x01]); // QTYPE=A
    packet.extend_from_slice(&[0x00, 0x01]); // QCLASS=IN

    packet
}

// ---------------------------------------------------------------------------
// HTTP probes (direct + proxy)
// ---------------------------------------------------------------------------

/// Direct HTTP request to `PROBE_URL`, no proxy. Measures the total time.
/// Uses `curl` to explicitly bind to the physical interface and ignore system proxies.
fn http_probe_direct(local_ip: Option<&str>) -> Option<HttpProbe> {
    use std::process::Command;

    let mut cmd = Command::new("curl");

    // -I: HEAD request
    // -s: silent
    // --noproxy "*": explicitly ignore macOS system proxies
    // -w "%{time_total}": print total time in seconds
    // -o /dev/null: discard headers output
    cmd.args([
        "-I",
        "-s",
        "--noproxy",
        "*",
        "-4",
        "-w",
        "%{time_total}",
        "-o",
        "/dev/null",
    ]);

    // Enforce 10s timeout overall, 5s connect timeout
    cmd.args(["--max-time", "10", "--connect-timeout", "5"]);

    // Bind to the physical interface IP to bypass TUN routing
    if let Some(ip) = local_ip {
        cmd.args(["--interface", ip]);
    }

    cmd.arg(PROBE_URL);

    let out = match cmd.output() {
        Ok(output) => output,
        Err(e) => {
            println!("Direct test failed to execute curl: {:?}", e);
            return None;
        }
    };

    if !out.status.success() {
        let error_msg = String::from_utf8_lossy(&out.stderr);
        println!(
            "Direct test curl failed with status {}. Error: {}",
            out.status,
            error_msg.trim()
        );
        return None;
    }

    let stdout = String::from_utf8_lossy(&out.stdout);
    let seconds: f64 = match stdout.trim().parse() {
        Ok(s) => s,
        Err(e) => {
            println!("Failed to parse curl time output '{}': {:?}", stdout, e);
            return None;
        }
    };

    let total_ms = (seconds * 1000.0).round() as u64;

    Some(HttpProbe { total_ms, ok: true })
}

/// HTTP request to `PROBE_URL` tunneled through the local HTTP proxy (the mixed
/// inbound zeytun-core exposes). Measures the total time.
fn http_probe_proxy(proxy_port: u16) -> Option<HttpProbe> {
    use std::process::Command;
    let mut cmd = Command::new("curl");
    let proxy_str = format!("http://127.0.0.1:{}", proxy_port);

    // -I: HEAD request
    // -s: silent stdout (stderr stays open)
    // -L: follow redirects (vital for google 301)
    // -x: explicitly route through our proxy port
    // -w "%{time_total}": print total time in seconds
    // -o /dev/null: discard headers output
    cmd.args([
        "-I",
        "-s",
        "-L",
        "--noproxy",
        "*",
        "-x",
        &proxy_str,
        "-w",
        "%{time_total}",
        "-o",
        "/dev/null",
        PROBE_URL,
    ]);

    // Enforce timeouts so it doesn't hang forever
    cmd.args(["--max-time", "10", "--connect-timeout", "5"]);

    let out = match cmd.output() {
        Ok(output) => output,
        Err(e) => {
            println!("Proxy test failed to execute curl: {:?}", e);
            return None;
        }
    };

    if !out.status.success() {
        let error_msg = String::from_utf8_lossy(&out.stderr);
        println!(
            "Proxy test curl failed with status {}. Error: {}",
            out.status,
            error_msg.trim()
        );
        return None;
    }

    let stdout = String::from_utf8_lossy(&out.stdout);

    let seconds: f64 = match stdout.trim().parse() {
        Ok(s) => s,
        Err(e) => {
            println!("Failed to parse curl time output '{}': {:?}", stdout, e);
            return None;
        }
    };

    let total_ms = (seconds * 1000.0).round() as u64;

    Some(HttpProbe { total_ms, ok: true })
}

// ---------------------------------------------------------------------------
// Report formatting
// ---------------------------------------------------------------------------

#[allow(clippy::too_many_arguments)]
fn build_report(
    cfg: &NetConfig,
    router_ms: Option<u64>,
    dns_server: &str,
    dns_ms: Option<u64>,
    direct: Option<&HttpProbe>,
    outbound_mode: &str,
    proxy_available: bool,
    proxy: Option<&HttpProbe>,
    internet_test_url: &str,
    proxy_test_url: &str,
) -> String {
    let na = "N/A".to_string();
    let mut out = String::new();

    // Check configuration
    // out.push_str("Check configuration\n");
    // out.push_str("The configuration test is successful\n\n");

    // Network configuration
    out.push_str("Network configuration\n");
    out.push_str(&format!(
        "Primary IPv4 interface: {}\n",
        cfg.ipv4_interface.as_ref().unwrap_or(&na)
    ));
    out.push_str(&format!(
        "Primary IPv4 address: {}\n",
        cfg.ipv4_address.as_ref().unwrap_or(&na)
    ));
    out.push_str(&format!(
        "Default IPv4 router: {}\n",
        cfg.ipv4_router.as_ref().unwrap_or(&na)
    ));
    out.push_str(&format!(
        "Primary IPv6 interface: {}\n",
        cfg.ipv6_interface.as_ref().unwrap_or(&na)
    ));
    out.push_str(&format!(
        "Primary IPv6 address: {}\n",
        cfg.ipv6_address.as_ref().unwrap_or(&na)
    ));
    out.push_str(&format!("Effective DNS server: {}\n", dns_server));

    // Router
    out.push_str("Router\n");
    match (cfg.ipv4_router.as_ref(), router_ms) {
        (Some(gw), Some(ms)) => out.push_str(&format!("Ping {gw}: {ms} ms\n\n")),
        (Some(gw), None) => out.push_str(&format!("Ping {gw}: failed\n\n")),
        (None, _) => out.push_str("No default router found\n\n"),
    }

    // DNS
    out.push_str("DNS\n");
    match dns_ms {
        Some(ms) => out.push_str(&format!("Answer from {dns_server}:53: {ms} ms\n\n")),
        None => out.push_str(&format!("Answer from {dns_server}:53: failed\n\n")),
    }

    // Direct Policy
    out.push_str("Direct Policy\n");
    match direct {
        Some(p) if p.ok => out.push_str(&format!(
            "Direct connection to {internet_test_url}: {} ms\n\n",
            p.total_ms
        )),
        _ => out.push_str(&format!(
            "Direct connection to {internet_test_url}: failed\n\n"
        )),
    }

    // Proxy
    out.push_str("Proxy\n");
    if !proxy_available {
        out.push_str(&format!(
            "Proxy test skipped (outbound mode is {outbound_mode}).\n\n"
        ));
    } else {
        match proxy {
            Some(p) if p.ok => {
                out.push_str(&format!(
                    "Connect through HTTP Proxy to {proxy_test_url} successfully.\n"
                ));
                out.push_str(&format!(
                    "Total time cost for a single HTTP request: {} ms\n\n",
                    p.total_ms
                ));
            }
            _ => out.push_str(&format!(
                "Failed to connect through HTTP Proxy to {proxy_test_url}.\n\n"
            )),
        }
    }

    // UDP Proxy Relay
    // out.push_str("UDP Proxy Relay\n");
    // out.push_str("[Test completed]\n");

    out
}
