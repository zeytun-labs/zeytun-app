use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ListenOptions {
    /// Listen address
    ///
    /// default:
    /// 0.0.0.0
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub listen: Option<String>,

    /// Listen port
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub listen_port: Option<u16>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub routing_mark: Option<u32>,

    /// Enable SO_REUSEPORT
    ///
    /// default: false
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reuse_port: Option<bool>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub netns: Option<String>,

    /// TCP Fast Open
    ///
    /// default: false
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tcp_fast_open: Option<bool>,

    /// TCP Multi Path
    ///
    /// default: false
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tcp_multi_path: Option<bool>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disable_tcp_keep_alive: Option<bool>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tcp_keep_alive: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tcp_keep_alive_interval: Option<String>,

    /// UDP fragment
    ///
    /// default: false
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub udp_fragment: Option<bool>,

    /// UDP timeout
    ///
    /// golang duration
    ///
    /// default: 5m
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub udp_timeout: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detour: Option<String>,
}
