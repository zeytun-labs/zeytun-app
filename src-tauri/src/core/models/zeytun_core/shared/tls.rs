use serde::{Deserialize, Serialize};

use crate::core::models::zeytun_core::shared::dial::DialOptions;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct InboundTlsOptions {
    pub enabled: bool,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub server_name: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub alpn: Option<Vec<String>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_version: Option<TlsVersion>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_version: Option<TlsVersion>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cipher_suites: Option<Vec<String>>,

    /// PEM cert content
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub certificate: Option<Vec<String>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub certificate_path: Option<Vec<String>>,

    /// PEM key content
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key: Option<Vec<String>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key_path: Option<Vec<String>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ech: Option<InboundEchOptions>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reality: Option<InboundRealityOptions>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub handshake_timeout: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub engine: Option<TlsEngine>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct InboundEchOptions {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key: Option<Vec<String>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct InboundRealityOptions {
    pub enabled: bool,

    pub handshake: InboundRealityHandshake,

    pub private_key: String,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub short_id: Option<Vec<String>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_time_difference: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct InboundRealityHandshake {
    pub server: String,

    pub server_port: u16,

    #[serde(flatten)]
    pub dial: DialOptions,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct OutboundTlsOptions {
    pub enabled: bool,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub server_name: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub insecure: Option<bool>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub alpn: Option<Vec<String>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_version: Option<TlsVersion>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_version: Option<TlsVersion>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cipher_suites: Option<Vec<String>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub certificate: Option<Vec<String>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub certificate_path: Option<Vec<String>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disable_sni: Option<bool>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ech: Option<OutboundEchOptions>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub utls: Option<OutboundUtlsOptions>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reality: Option<OutboundRealityOptions>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fragment: Option<bool>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fragment_fallback_delay: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub record_fragment: Option<bool>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub handshake_timeout: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub client_certificate: Option<Vec<String>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub client_certificate_path: Option<Vec<String>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub client_key: Option<Vec<String>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub client_key_path: Option<Vec<String>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub engine: Option<TlsEngine>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spoof: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spoof_method: Option<SpoofMethod>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TlsVersion {
    #[serde(rename = "1.0")]
    V1_0,

    #[serde(rename = "1.1")]
    V1_1,

    #[serde(rename = "1.2")]
    V1_2,

    #[serde(rename = "1.3")]
    V1_3,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TlsEngine {
    Go,
    Apple,
    Windows,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct OutboundEchOptions {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub config: Option<Vec<String>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub config_path: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub query_server_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct OutboundUtlsOptions {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fingerprint: Option<OutboundUtlsFingerprint>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OutboundUtlsFingerprint {
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

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct OutboundRealityOptions {
    pub enabled: bool,

    pub public_key: String,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub short_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SpoofMethod {
    WrongSequence,
    WrongChecksum,
    WrongAck,
    WrongMd5,
    WrongTimestamp,
}
