use zeytun_config::{
    parse_share_proxy, share_to_link, Http as ZHttp, Hysteria2 as ZHysteria2,
    Shadowsocks as ZShadowsocks, ShareProtocol, ShareProxy, Socks as ZSocks, Tls as ZTls,
    Trojan as ZTrojan, Tuic as ZTuic, Vless as ZVless, Vmess as ZVmess,
};

use crate::core::{
    dto::{Proxy, ProxyOrigin},
    models::proxy::{
        Fingerprint, HttpProtocol, Hysteria2Protocol, Protocol, ProxyServer, ShadowsocksProtocol,
        SocksProtocol, Tls, TrojanProtocol, TuicProtocol, VlessProtocol, VmessProtocol,
    },
};

/// Upper bound on a single share link. A real link is at most a few hundred
/// bytes; anything larger is either hostile or a malformed paste, and parsing
/// megabytes of it only wastes CPU. ponytail: raise only if a legit source
/// ever emits links near this size.
const MAX_LINK_LEN: usize = 8 * 1024;

pub fn proxy_from_link(link: &str, origin: ProxyOrigin, tag: String) -> Result<Proxy, String> {
    if link.len() > MAX_LINK_LEN {
        return Err(format!("proxy link exceeds {MAX_LINK_LEN} bytes"));
    }
    let server = parse_proxy_server(link, tag.clone())?;
    Ok(Proxy {
        config: serde_json::to_value(&server).ok(),
        tag,
        origin,
        title: server.name,
        protocol: server.protocol.name().to_string(),
        link: link.trim().to_string(),
        enabled: true,
        transport: server.protocol.transport(),
    })
}

pub fn proxy_to_proxy_server(proxy: &Proxy) -> Result<ProxyServer, String> {
    if let Some(config) = &proxy.config {
        if let Ok(mut server) = serde_json::from_value::<ProxyServer>(config.clone()) {
            server.tag = proxy.tag.clone();
            server.name = proxy.title.clone();
            return Ok(server);
        }
    }
    let mut server = parse_proxy_server(&proxy.link, proxy.tag.clone())?;
    server.name = proxy.title.clone();
    Ok(server)
}

/// Share-link for Copy / QR. Prefers live `config` over stale stored `link`.
pub fn proxy_to_link(proxy: &Proxy) -> Result<String, String> {
    let server = proxy_to_proxy_server(proxy)?;
    let node = proxy_server_to_share(server)?;
    share_to_link(&node).map_err(|e| e.to_string())
}

fn proxy_server_to_share(server: ProxyServer) -> Result<ShareProxy, String> {
    let protocol = match server.protocol {
        Protocol::Vless(v) => ShareProtocol::Vless(ZVless {
            uuid: v.uuid,
            flow: v.flow,
            packet_encoding: v.packet_encoding,
            network: v.network,
            transport: v.transport,
            tls: v.tls.map(unmap_tls),
        }),
        Protocol::Vmess(v) => ShareProtocol::Vmess(ZVmess {
            uuid: v.uuid,
            alter_id: v.alter_id,
            security: v.security,
            packet_encoding: v.packet_encoding,
            network: v.network,
            transport: v.transport,
            tls: v.tls.map(unmap_tls),
        }),
        Protocol::Trojan(t) => ShareProtocol::Trojan(ZTrojan {
            password: t.password,
            network: t.network,
            transport: t.transport,
            tls: t.tls.map(unmap_tls),
        }),
        Protocol::Shadowsocks(s) => ShareProtocol::Shadowsocks(ZShadowsocks {
            method: s.encryption,
            password: s.password,
            plugin: s.plugin,
            plugin_opts: s.plugin_args,
            udp_over_tcp: s.udp_over_tcp,
        }),
        Protocol::Hysteria2(h) => ShareProtocol::Hysteria2(ZHysteria2 {
            password: h.password,
            up_mbps: h.up_mbps,
            down_mbps: h.down_mbps,
            server_ports: h.server_ports,
            hop_interval: h.hop_interval,
            obfs: None, // ponytail: app stores only obf_password; add obfs type when UI has it
            obfs_password: h.obf_password,
            tls: unmap_tls(h.tls),
        }),
        Protocol::Tuic(t) => ShareProtocol::Tuic(ZTuic {
            uuid: t.uuid,
            password: t.password,
            congestion_control: t.congestion_control,
            udp_relay_mode: t.udp_relay_mode,
            udp_over_stream: t.udp_over_stream,
            zero_rtt_handshake: t.zero_rtt_handshake,
            heartbeat: t.heartbeat,
            tls: unmap_tls(t.tls),
        }),
        Protocol::Socks(s) => ShareProtocol::Socks(ZSocks {
            version: s.version,
            username: s.username,
            password: s.password,
        }),
        Protocol::Http(h) => ShareProtocol::Http(ZHttp {
            username: h.username,
            password: h.password,
            path: None,
            tls: h.tls.map(unmap_tls),
        }),
        Protocol::Chain(_) => {
            return Err("chain proxies cannot be exported as a share link".into());
        }
    };

    Ok(ShareProxy {
        name: server.name,
        address: server.address,
        port: server.port,
        detour: server.detour,
        protocol,
    })
}

fn unmap_tls(t: Tls) -> ZTls {
    ZTls {
        enabled: true,
        allow_insecure: t.allow_insecure,
        sni: t.sni,
        alpn: t.alpn,
        disable_sni: false,
        fingerprint: t.fingerprint.as_ref().map(fingerprint_str),
        reality_pbk: t.reality_pbk,
        reality_sid: t.reality_sid,
    }
}

fn fingerprint_str(fp: &Fingerprint) -> String {
    match fp {
        Fingerprint::Chrome => "chrome",
        Fingerprint::Firefox => "firefox",
        Fingerprint::Edge => "edge",
        Fingerprint::Qq => "qq",
        Fingerprint::Ios => "ios",
        Fingerprint::Android => "android",
        Fingerprint::Random => "random",
        Fingerprint::Randomized => "randomized",
        Fingerprint::ThreeSixty => "360",
    }
    .into()
}

fn parse_proxy_server(link: &str, tag: String) -> Result<ProxyServer, String> {
    let node = parse_share_proxy(link.trim()).map_err(|e| e.to_string())?;
    Ok(share_to_proxy_server(node, tag))
}

fn share_to_proxy_server(node: ShareProxy, tag: String) -> ProxyServer {
    let protocol = match node.protocol {
        ShareProtocol::Vless(v) => Protocol::Vless(VlessProtocol {
            uuid: v.uuid,
            flow: v.flow,
            packet_encoding: v.packet_encoding,
            network: v.network,
            transport: v.transport,
            tls: v.tls.map(map_tls),
            mux: Some(false),
            tcp_brutal: Some(false),
            brutal_dl_speed: None,
            brutal_up_speed: None,
        }),
        ShareProtocol::Vmess(v) => Protocol::Vmess(VmessProtocol {
            uuid: v.uuid,
            alter_id: v.alter_id,
            packet_encoding: v.packet_encoding,
            security: v.security,
            network: v.network,
            transport: v.transport,
            tls: v.tls.map(map_tls),
            mux: Some(false),
            tcp_brutal: Some(false),
            brutal_dl_speed: None,
            brutal_up_speed: None,
        }),
        ShareProtocol::Trojan(t) => Protocol::Trojan(TrojanProtocol {
            password: t.password,
            network: t.network,
            transport: t.transport,
            tls: t.tls.map(map_tls),
            mux: Some(false),
            tcp_brutal: Some(false),
            brutal_dl_speed: None,
            brutal_up_speed: None,
        }),
        ShareProtocol::Shadowsocks(s) => Protocol::Shadowsocks(ShadowsocksProtocol {
            encryption: s.method,
            password: s.password,
            plugin: s.plugin,
            plugin_args: s.plugin_opts,
            udp_over_tcp: s.udp_over_tcp,
            mux: Some(false),
            tcp_brutal: Some(false),
            brutal_dl_speed: None,
            brutal_up_speed: None,
        }),
        ShareProtocol::Hysteria2(h) => Protocol::Hysteria2(Hysteria2Protocol {
            server_ports: h.server_ports,
            hop_interval: h.hop_interval,
            up_mbps: h.up_mbps,
            down_mbps: h.down_mbps,
            obf_password: h.obfs_password,
            password: h.password,
            tls: map_tls(h.tls),
        }),
        ShareProtocol::Tuic(t) => Protocol::Tuic(TuicProtocol {
            uuid: t.uuid,
            password: t.password,
            congestion_control: t.congestion_control,
            udp_relay_mode: t.udp_relay_mode,
            udp_over_stream: t.udp_over_stream,
            zero_rtt_handshake: t.zero_rtt_handshake,
            heartbeat: t.heartbeat,
            tls: map_tls(t.tls),
        }),
        ShareProtocol::Socks(s) => Protocol::Socks(SocksProtocol {
            version: s.version,
            username: s.username,
            password: s.password,
        }),
        ShareProtocol::Http(h) => Protocol::Http(HttpProtocol {
            username: h.username,
            password: h.password,
            tls: h.tls.map(map_tls),
        }),
    };

    ProxyServer {
        tag,
        name: node.name,
        address: node.address,
        port: node.port,
        advanced: None,
        reuse_address: None,
        tcp_fast_open: None,
        udp_fragment: None,
        tcp_multi_path: None,
        connect_timeout: None,
        tls_disable_sni: None,
        tls_min_version: None,
        tls_max_version: None,
        tls_enable_ech: None,
        tls_ech_config: None,
        tls_certificate_sha256: None,
        tls_client_cert: None,
        tls_client_key: None,
        detour: node.detour,
        protocol,
    }
}

fn map_tls(t: ZTls) -> Tls {
    Tls {
        allow_insecure: t.allow_insecure,
        certificate: None,
        sni: t.sni,
        alpn: t.alpn,
        fragment: false,
        fallback_delay: None,
        record_fragment: false,
        fingerprint: t.fingerprint.as_deref().and_then(parse_fingerprint),
        reality_pbk: t.reality_pbk,
        reality_sid: t.reality_sid,
    }
}

fn parse_fingerprint(value: &str) -> Option<Fingerprint> {
    match value.to_ascii_lowercase().as_str() {
        "chrome" => Some(Fingerprint::Chrome),
        "firefox" => Some(Fingerprint::Firefox),
        "edge" => Some(Fingerprint::Edge),
        "qq" => Some(Fingerprint::Qq),
        "ios" => Some(Fingerprint::Ios),
        "android" => Some(Fingerprint::Android),
        "random" => Some(Fingerprint::Random),
        "randomized" => Some(Fingerprint::Randomized),
        "360" => Some(Fingerprint::ThreeSixty),
        _ => None,
    }
}
