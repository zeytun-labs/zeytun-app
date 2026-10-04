use serde::{Deserialize, Serialize};

use crate::core::models::zeytun_core::shared::{
    dial::DialOptions, mux::MultiplexOptions, network::NetworkOptions, udp_over_tcp::UdpOverTcp,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShadowsocksOutbound {
    pub server: String,
    pub server_port: u16,

    #[serde(flatten)]
    pub dial: DialOptions,

    pub method: ShadowsocksMethod,

    pub password: String,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plugin: Option<ShadowsocksPlugin>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plugin_opts: Option<String>,

    /// tcp / udp
    ///
    /// both enabled by default
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub network: Option<NetworkOptions>,

    /// conflicts with multiplex
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub udp_over_tcp: Option<UdpOverTcp>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub multiplex: Option<MultiplexOptions>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ShadowsocksPlugin {
    ObfsLocal,
    V2rayPlugin,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ShadowsocksMethod {
    #[serde(rename = "2022-blake3-aes-128-gcm")]
    Blake3Aes128Gcm2022,

    #[serde(rename = "2022-blake3-aes-256-gcm")]
    Blake3Aes256Gcm2022,

    #[serde(rename = "2022-blake3-chacha20-poly1305")]
    Blake3Chacha20Poly1305_2022,

    #[serde(rename = "none")]
    None,

    #[serde(rename = "aes-128-gcm")]
    Aes128Gcm,

    #[serde(rename = "aes-192-gcm")]
    Aes192Gcm,

    #[serde(rename = "aes-256-gcm")]
    Aes256Gcm,

    #[serde(rename = "chacha20-ietf-poly1305")]
    Chacha20IetfPoly1305,

    #[serde(rename = "xchacha20-ietf-poly1305")]
    Xchacha20IetfPoly1305,

    #[serde(rename = "aes-128-ctr")]
    Aes128Ctr,

    #[serde(rename = "aes-192-ctr")]
    Aes192Ctr,

    #[serde(rename = "aes-256-ctr")]
    Aes256Ctr,

    #[serde(rename = "aes-128-cfb")]
    Aes128Cfb,

    #[serde(rename = "aes-192-cfb")]
    Aes192Cfb,

    #[serde(rename = "aes-256-cfb")]
    Aes256Cfb,

    #[serde(rename = "rc4-md5")]
    Rc4Md5,

    #[serde(rename = "chacha20-ietf")]
    Chacha20Ietf,

    #[serde(rename = "xchacha20")]
    Xchacha20,
}
