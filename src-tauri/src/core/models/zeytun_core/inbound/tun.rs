use serde::{Deserialize, Serialize};

use crate::core::models::zeytun_core::shared::listen::ListenOptions;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TunInbound {
    #[serde(flatten)]
    pub listen: ListenOptions,

    /// Virtual interface name
    ///
    /// auto selected if empty
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub interface_name: Option<String>,

    /// IPv4 / IPv6 prefixes
    ///
    /// e.g.
    /// ["172.19.0.1/30", "fdfe:dcba:9876::1/126"]
    pub address: Vec<String>,

    /// Maximum transmission unit
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mtu: Option<u32>,

    /// gvisor | mixed | system
    ///
    /// default: system
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stack: Option<TunStack>,

    /// disabled | native | hijack
    ///
    /// default: hijack
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dns_mode: Option<TunDnsMode>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auto_route: Option<bool>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub strict_route: Option<bool>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auto_redirect: Option<bool>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auto_redirect_input_mark: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auto_redirect_output_mark: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auto_redirect_reset_mark: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auto_redirect_nfqueue: Option<u32>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auto_redirect_iproute2_fallback_rule_index: Option<u32>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exclude_mptcp: Option<bool>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub loopback_address: Option<Vec<String>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub route_address: Option<Vec<String>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub route_exclude_address: Option<Vec<String>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub route_address_set: Option<Vec<String>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub route_exclude_address_set: Option<Vec<String>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub endpoint_independent_nat: Option<bool>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub udp_timeout: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub include_interface: Option<Vec<String>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exclude_interface: Option<Vec<String>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub include_uid: Option<Vec<u32>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub include_uid_range: Option<Vec<String>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exclude_uid: Option<Vec<u32>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exclude_uid_range: Option<Vec<String>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub include_android_user: Option<Vec<u32>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub include_package: Option<Vec<String>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exclude_package: Option<Vec<String>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub include_mac_address: Option<Vec<String>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exclude_mac_address: Option<Vec<String>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub platform: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TunStack {
    System,
    Gvisor,
    Mixed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TunDnsMode {
    Disabled,
    Native,
    Hijack,
}
