use serde::{Deserialize, Serialize};

use crate::core::models::zeytun_core::shared::{
    dial::DialOptions, network::NetworkOptions, quic::QuicOptions, tls::OutboundTlsOptions,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TuicOutbound {
    pub server: String,
    pub server_port: u16,

    #[serde(flatten)]
    pub dial: DialOptions,

    pub uuid: String,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub password: Option<String>,

    /// cubic | new_reno | bbr
    ///
    /// default: cubic
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub congestion_control: Option<TuicCongestionControl>,

    /// native | quic
    ///
    /// default: native
    ///
    /// conflicts with udp_over_stream
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub udp_relay_mode: Option<TuicUdpRelayMode>,

    /// conflicts with udp_relay_mode
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub udp_over_stream: Option<bool>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub zero_rtt_handshake: Option<bool>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub heartbeat: Option<String>,

    /// tcp / udp
    ///
    /// both enabled by default
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub network: Option<NetworkOptions>,

    /// required
    pub tls: OutboundTlsOptions,

    #[serde(flatten)]
    pub quic: QuicOptions,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TuicUdpRelayMode {
    Native,
    Quic,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TuicCongestionControl {
    Cubic,
    NewReno,
    Bbr,
}
