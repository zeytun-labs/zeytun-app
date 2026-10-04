use serde::{Deserialize, Serialize};

use crate::core::models::zeytun_core::shared::{
    dial::DialOptions, mux::MultiplexOptions, network::NetworkOptions,
    packet_encoding::PacketEncoding, tls::OutboundTlsOptions, transport::V2RayTransport,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VmessOutbound {
    pub server: String,
    pub server_port: u16,

    #[serde(flatten)]
    pub dial: DialOptions,

    pub uuid: String,

    /// auto | aes-128-gcm | chacha20-poly1305 | none
    ///
    /// default: auto
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub security: Option<VmessSecurity>,

    /// default: 0
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub alter_id: Option<u32>,

    /// default: false
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub global_padding: Option<bool>,

    /// default: false
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub authenticated_length: Option<bool>,

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
    pub transport: Option<V2RayTransport>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub multiplex: Option<MultiplexOptions>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum VmessSecurity {
    Auto,

    Zero,

    Aes128Gcm,

    Chacha20Poly1305,

    None,
}
