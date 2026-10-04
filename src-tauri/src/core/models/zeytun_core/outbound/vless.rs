use serde::{Deserialize, Serialize};

use crate::core::models::zeytun_core::shared::{
    dial::DialOptions, mux::MultiplexOptions, network::NetworkOptions,
    packet_encoding::PacketEncoding, tls::OutboundTlsOptions, transport::V2RayTransport,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VlessOutbound {
    pub server: String,
    pub server_port: u16,

    #[serde(flatten)]
    pub dial: DialOptions,

    pub uuid: String,
    /// xtls-rprx-vision
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub flow: Option<VlessFlow>,

    /// tcp / udp
    ///
    /// both enabled by default
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub network: Option<NetworkOptions>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tls: Option<OutboundTlsOptions>,

    /// packetaddr / xudp
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub packet_encoding: Option<PacketEncoding>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub multiplex: Option<MultiplexOptions>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transport: Option<V2RayTransport>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum VlessFlow {
    XtlsRprxVision,
}
