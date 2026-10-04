use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DialOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detour: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub bind_interface: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub inet4_bind_address: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub inet6_bind_address: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub bind_address_no_port: Option<bool>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub routing_mark: Option<RoutingMark>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub reuse_addr: Option<bool>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub netns: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub connect_timeout: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub tcp_fast_open: Option<bool>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub tcp_multi_path: Option<bool>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub disable_tcp_keep_alive: Option<bool>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub tcp_keep_alive: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub tcp_keep_alive_interval: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub udp_fragment: Option<bool>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub domain_resolver: Option<DomainResolver>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub network_strategy: Option<NetworkStrategy>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub network_type: Option<Vec<NetworkType>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub fallback_network_type: Option<Vec<NetworkType>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub fallback_delay: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NetworkStrategy {
    Default,
    Hybrid,
    Fallback, // core network dial strategy (not policy)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NetworkType {
    Wifi,
    Cellular,
    Ethernet,
    Other,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum RoutingMark {
    Number(u32),
    HexString(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum DomainResolver {
    Server(String),
    Object(serde_json::Value),
}
