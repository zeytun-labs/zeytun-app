use serde::{Deserialize, Serialize};

use crate::core::models::zeytun_core::shared::{
    dial::DialOptions, mux::MultiplexOptions, network::NetworkOptions, tls::OutboundTlsOptions,
    transport::V2RayTransport,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrojanOutbound {
    pub server: String,
    pub server_port: u16,

    #[serde(flatten)]
    pub dial: DialOptions,

    pub password: String,

    /// tcp / udp
    ///
    /// both enabled by default
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub network: Option<NetworkOptions>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tls: Option<OutboundTlsOptions>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub multiplex: Option<MultiplexOptions>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transport: Option<V2RayTransport>,
}
