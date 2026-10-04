use serde::{Deserialize, Serialize};

use crate::core::models::zeytun_core::shared::{
    dial::DialOptions, network::NetworkOptions, quic::QuicOptions, tls::OutboundTlsOptions,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Hysteria2Outbound {
    pub server: String,
    pub server_port: u16,

    #[serde(flatten)]
    pub dial: DialOptions,

    /// conflicts with server_port + realm
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub server_ports: Option<Vec<String>>,

    /// default: 30s
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hop_interval: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hop_interval_max: Option<String>,

    /// max upload bandwidth (Mbps)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub up_mbps: Option<u32>,

    /// max download bandwidth (Mbps)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub down_mbps: Option<u32>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub obfs: Option<Hysteria2Obfs>,

    pub password: String,

    /// tcp / udp
    ///
    /// both enabled by default
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub network: Option<NetworkOptions>,

    /// required
    pub tls: OutboundTlsOptions,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub brutal_debug: Option<bool>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bbr_profile: Option<BbrProfile>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub realm: Option<Hysteria2Realm>,

    #[serde(flatten)]
    pub quic: QuicOptions,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Hysteria2Network {
    Tcp,
    Udp,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Hysteria2Obfs {
    /// only salamander supported
    #[serde(rename = "type")]
    pub obfs_type: Hysteria2ObfsType,

    pub password: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Hysteria2ObfsType {
    Salamander,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BbrProfile {
    Conservative,
    Standard,
    Aggressive,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Hysteria2Realm {
    pub server_url: String,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub token: Option<String>,

    pub realm_id: String,

    pub stun_servers: Vec<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub http_client: Option<serde_json::Value>,
}
