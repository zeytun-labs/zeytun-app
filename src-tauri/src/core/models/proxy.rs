use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyServer {
    pub tag: String,
    pub name: String,
    #[serde(default)]
    pub address: String,
    #[serde(default)]
    pub port: u16,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detour: Option<String>,

    // Advanced dial fields
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub advanced: Option<bool>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reuse_address: Option<bool>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tcp_fast_open: Option<bool>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub udp_fragment: Option<bool>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tcp_multi_path: Option<bool>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub connect_timeout: Option<u32>,

    // Advanced TLS fields (stored flat, matching frontend naming)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tls_disable_sni: Option<bool>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tls_min_version: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tls_max_version: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tls_enable_ech: Option<bool>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tls_ech_config: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tls_certificate_sha256: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tls_client_cert: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tls_client_key: Option<String>,

    #[serde(flatten)]
    pub protocol: Protocol,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum Protocol {
    Socks(SocksProtocol),
    Http(HttpProtocol),
    Shadowsocks(ShadowsocksProtocol),
    Trojan(TrojanProtocol),
    Hysteria2(Hysteria2Protocol),
    Tuic(TuicProtocol),
    Vless(VlessProtocol),
    Vmess(VmessProtocol),
    Chain(ChainProtocol),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChainProtocol {
    pub proxies: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TuicProtocol {
    pub uuid: String,

    pub password: String,

    pub congestion_control: String,

    pub udp_relay_mode: String,

    pub udp_over_stream: bool,

    pub zero_rtt_handshake: bool,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub heartbeat: Option<String>,

    pub tls: Tls,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Hysteria2Protocol {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub server_ports: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hop_interval: Option<String>,

    pub up_mbps: u32,

    pub down_mbps: u32,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub obf_password: Option<String>,

    pub password: String,

    pub tls: Tls,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VmessProtocol {
    pub uuid: String,

    pub alter_id: u32,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub packet_encoding: Option<String>,

    pub security: String,

    pub network: String,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transport: Option<TransportSettings>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tls: Option<Tls>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mux: Option<bool>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tcp_brutal: Option<bool>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub brutal_dl_speed: Option<u32>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub brutal_up_speed: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VlessProtocol {
    pub uuid: String,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub flow: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub packet_encoding: Option<String>,

    pub network: String,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transport: Option<TransportSettings>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tls: Option<Tls>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mux: Option<bool>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tcp_brutal: Option<bool>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub brutal_dl_speed: Option<u32>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub brutal_up_speed: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrojanProtocol {
    pub password: String,

    pub network: String,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transport: Option<TransportSettings>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tls: Option<Tls>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mux: Option<bool>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tcp_brutal: Option<bool>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub brutal_dl_speed: Option<u32>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub brutal_up_speed: Option<u32>,
}

// Phase1: transport ownership lives in zeytun-link (single source for xhttp knobs).
pub use zeytun_config::{
    DownloadSettings as XhttpDownloadSettings, Transport as TransportSettings,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShadowsocksProtocol {
    pub encryption: String,

    pub password: String,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plugin: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plugin_args: Option<String>,

    pub udp_over_tcp: bool,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mux: Option<bool>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tcp_brutal: Option<bool>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub brutal_dl_speed: Option<u32>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub brutal_up_speed: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SocksProtocol {
    pub version: u8,
    pub username: String,
    pub password: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpProtocol {
    pub username: String,
    pub password: String,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tls: Option<Tls>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tls {
    pub allow_insecure: bool,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub certificate: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sni: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub alpn: Option<Vec<String>>,

    pub fragment: bool,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fallback_delay: Option<String>,

    pub record_fragment: bool,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fingerprint: Option<Fingerprint>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reality_pbk: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reality_sid: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Fingerprint {
    Chrome,
    Firefox,
    Edge,
    Qq,
    Ios,
    Android,
    Random,
    Randomized,

    #[serde(rename = "360")]
    ThreeSixty,
}

impl Protocol {
    pub fn name(&self) -> &'static str {
        match self {
            Protocol::Socks(_) => "socks",
            Protocol::Http(_) => "http",
            Protocol::Shadowsocks(_) => "shadowsocks",
            Protocol::Trojan(_) => "trojan",
            Protocol::Hysteria2(_) => "hysteria2",
            Protocol::Tuic(_) => "tuic",
            Protocol::Vless(_) => "vless",
            Protocol::Vmess(_) => "vmess",
            Protocol::Chain(_) => "chain",
        }
    }

    pub fn transport(&self) -> Option<String> {
        match self {
            Protocol::Vless(v) => Some(v.network.clone()),
            Protocol::Vmess(v) => Some(v.network.clone()),
            Protocol::Trojan(t) => Some(t.network.clone()),
            _ => None,
        }
    }
}
