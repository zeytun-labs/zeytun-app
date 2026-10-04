use serde::{Deserialize, Serialize};

use crate::core::models::zeytun_core::shared::{
    dial::DialOptions, network::NetworkOptions, udp_over_tcp::UdpOverTcp,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SocksOutbound {
    pub server: String,
    pub server_port: u16,

    #[serde(flatten)]
    pub dial: DialOptions,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<SocksVersion>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub password: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub network: Option<NetworkOptions>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub udp_over_tcp: Option<UdpOverTcp>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SocksVersion {
    #[serde(rename = "4")]
    V4,

    #[serde(rename = "4a")]
    V4a,

    #[serde(rename = "5")]
    V5,
}
