use std::collections::HashMap;

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value;

use crate::core::models::zeytun_core::shared::dial::{
    DomainResolver, NetworkStrategy, NetworkType as DialNetworkType, RoutingMark,
};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Route {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rules: Option<Vec<RouteRule>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rule_set: Option<Vec<RouteRuleSet>>,

    #[serde(default, rename = "final", skip_serializing_if = "Option::is_none")]
    pub final_outbound: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auto_detect_interface: Option<bool>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub override_android_vpn: Option<bool>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_interface: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_mark: Option<RoutingMark>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub find_process: Option<bool>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub find_neighbor: Option<bool>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dhcp_lease_files: Option<Vec<String>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_http_client: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_domain_resolver: Option<DomainResolver>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_network_strategy: Option<NetworkStrategy>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_network_type: Option<Vec<DialNetworkType>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_fallback_network_type: Option<Vec<DialNetworkType>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_fallback_delay: Option<String>,

    /// Hold unmatched TCP; timeout uses final. Core: route.connection_ask
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub connection_ask: Option<ConnectionAskOptions>,

    /// User temp + permanent rules (live store; cold start from config).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub live_rules: Option<LiveRulesOptions>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ConnectionAskOptions {
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub enabled: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout_ms: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group_by: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on_timeout: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LiveRulesOptions {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub temp: Vec<LiveRuleEntry>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub permanent: Vec<LiveRuleEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LiveRuleEntry {
    pub id: u64,
    #[serde(default, skip_serializing_if = "is_zero_i64")]
    pub expires_at: i64,
    pub rule: Value,
}

fn is_zero_i64(v: &i64) -> bool {
    *v == 0
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum RouteRuleSet {
    Inline(InlineRouteRuleSet),
    Local(LocalRouteRuleSet),
    Remote(RemoteRouteRuleSet),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InlineRouteRuleSet {
    #[serde(rename = "type", default, skip_serializing_if = "Option::is_none")]
    pub rule_set_type: Option<InlineRouteRuleSetType>,

    pub tag: String,

    pub rules: Vec<HeadlessRule>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalRouteRuleSet {
    #[serde(rename = "type")]
    pub rule_set_type: LocalRouteRuleSetType,

    pub tag: String,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub format: Option<RuleSetFormat>,

    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteRouteRuleSet {
    #[serde(rename = "type")]
    pub rule_set_type: RemoteRouteRuleSetType,

    pub tag: String,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub format: Option<RuleSetFormat>,

    pub url: String,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub http_client: Option<HttpClientRef>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub download_detour: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub update_interval: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum InlineRouteRuleSetType {
    Inline,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LocalRouteRuleSetType {
    Local,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RemoteRouteRuleSetType {
    Remote,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RuleSetFormat {
    Source,
    Binary,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum HttpClientRef {
    Tag(String),
    Fields(Value),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
// ponytail: boxed variants would change the serde wire layout of the config
// compile path; guarded by core_config_conformance, so suppress instead.
#[allow(clippy::large_enum_variant)]
pub enum HeadlessRule {
    Default(HeadlessDefaultRule),
    Logical(HeadlessLogicalRule),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeadlessDefaultRule {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub query_type: Option<Vec<QueryType>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub network: Option<Vec<HeadlessNetworkType>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub domain: Option<Vec<String>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub domain_suffix: Option<Vec<String>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub domain_keyword: Option<Vec<String>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub domain_regex: Option<Vec<String>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_ip_cidr: Option<Vec<String>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ip_cidr: Option<Vec<String>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_port: Option<Vec<u16>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_port_range: Option<Vec<String>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub port: Option<Vec<u16>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub port_range: Option<Vec<String>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub process_name: Option<Vec<String>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub process_path: Option<Vec<String>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub process_path_regex: Option<Vec<String>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub package_name: Option<Vec<String>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub package_name_regex: Option<Vec<String>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub network_type: Option<Vec<ClientNetworkType>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub network_is_expensive: Option<bool>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub network_is_constrained: Option<bool>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub network_interface_address: Option<HashMap<String, Vec<String>>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_interface_address: Option<Vec<String>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wifi_ssid: Option<Vec<String>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wifi_bssid: Option<Vec<String>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub invert: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeadlessLogicalRule {
    #[serde(rename = "type")]
    pub rule_type: LogicalType,

    pub mode: LogicalMode,

    pub rules: Vec<HeadlessRule>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub invert: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum QueryType {
    Number(u16),
    Name(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HeadlessNetworkType {
    Tcp,
    Udp,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
// ponytail: boxed variants would change the serde wire layout of the config
// compile path; guarded by core_config_conformance, so suppress instead.
#[allow(clippy::large_enum_variant)]
pub enum RouteRule {
    Default(DefaultRouteRule),
    Logical(LogicalRouteRule),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DefaultRouteRule {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inbound: Option<Vec<String>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ip_version: Option<IpVersion>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub network: Option<Vec<NetworkType>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auth_user: Option<Vec<String>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub protocol: Option<Vec<String>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub client: Option<Vec<String>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub domain: Option<Vec<String>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub domain_suffix: Option<Vec<String>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub domain_keyword: Option<Vec<String>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub domain_regex: Option<Vec<String>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_ip_cidr: Option<Vec<String>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_ip_is_private: Option<bool>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ip_cidr: Option<Vec<String>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ip_is_private: Option<bool>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_port: Option<Vec<u16>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_port_range: Option<Vec<String>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub port: Option<Vec<u16>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub port_range: Option<Vec<String>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub process_name: Option<Vec<String>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub process_path: Option<Vec<String>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub process_path_regex: Option<Vec<String>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub package_name: Option<Vec<String>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub package_name_regex: Option<Vec<String>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user: Option<Vec<String>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_id: Option<Vec<u32>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub clash_mode: Option<ClashMode>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub network_type: Option<Vec<ClientNetworkType>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub network_is_expensive: Option<bool>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub network_is_constrained: Option<bool>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub interface_address: Option<HashMap<String, Vec<String>>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub network_interface_address: Option<HashMap<String, Vec<String>>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_interface_address: Option<Vec<String>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wifi_ssid: Option<Vec<String>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wifi_bssid: Option<Vec<String>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preferred_by: Option<Vec<String>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_mac_address: Option<Vec<String>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_hostname: Option<Vec<String>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rule_set: Option<Vec<String>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rule_set_ip_cidr_match_source: Option<bool>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub invert: Option<bool>,

    #[serde(flatten)]
    pub action: RuleAction,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogicalRouteRule {
    #[serde(rename = "type")]
    pub rule_type: LogicalType,

    pub mode: LogicalMode,

    pub rules: Vec<RouteRule>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub invert: Option<bool>,

    #[serde(flatten)]
    pub action: RuleAction,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LogicalType {
    Logical,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LogicalMode {
    And,
    Or,
}

#[derive(Debug, Clone)]
pub enum IpVersion {
    V4,
    V6,
}

impl Serialize for IpVersion {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            Self::V4 => serializer.serialize_u8(4),
            Self::V6 => serializer.serialize_u8(6),
        }
    }
}

impl<'de> Deserialize<'de> for IpVersion {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum RawIpVersion {
            Number(u8),
            Name(String),
        }

        match RawIpVersion::deserialize(deserializer)? {
            RawIpVersion::Number(4) => Ok(Self::V4),
            RawIpVersion::Number(6) => Ok(Self::V6),
            RawIpVersion::Number(other) => Err(serde::de::Error::custom(format!(
                "invalid ip_version value: {other}"
            ))),
            RawIpVersion::Name(name) => match name.to_ascii_lowercase().as_str() {
                "4" | "ipv4" => Ok(Self::V4),
                "6" | "ipv6" => Ok(Self::V6),
                other => Err(serde::de::Error::custom(format!(
                    "invalid ip_version value: {other}"
                ))),
            },
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum NetworkType {
    Tcp,
    Udp,
    Icmp,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ClashMode {
    Direct,
    Global,
    Rule,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ClientNetworkType {
    Wifi,
    Cellular,
    Ethernet,
    Other,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RouteOptionsAction {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub override_address: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub override_port: Option<u16>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub network_strategy: Option<NetworkStrategy>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub network_type: Option<Vec<DialNetworkType>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fallback_network_type: Option<Vec<DialNetworkType>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fallback_delay: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub udp_disable_domain_unmapping: Option<bool>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub udp_connect: Option<bool>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub udp_timeout: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tls_fragment: Option<bool>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tls_fragment_fallback_delay: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tls_record_fragment: Option<bool>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tls_spoof: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tls_spoof_method: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DomainStrategy {
    PreferIpv4,
    PreferIpv6,
    Ipv4Only,
    Ipv6Only,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "kebab-case")]
pub enum RuleAction {
    Route {
        outbound: String,

        #[serde(flatten)]
        route_options: RouteOptionsAction,
    },

    Reject {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        method: Option<RejectMethod>,

        #[serde(default, skip_serializing_if = "Option::is_none")]
        no_drop: Option<bool>,
    },

    HijackDns,

    RouteOptions {
        #[serde(flatten)]
        route_options: RouteOptionsAction,
    },

    Sniff {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        sniffer: Option<Vec<String>>,

        #[serde(default, skip_serializing_if = "Option::is_none")]
        timeout: Option<String>,
    },

    Resolve {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        server: Option<String>,

        #[serde(default, skip_serializing_if = "Option::is_none")]
        strategy: Option<DomainStrategy>,

        #[serde(default, skip_serializing_if = "Option::is_none")]
        disable_cache: Option<bool>,

        #[serde(default, skip_serializing_if = "Option::is_none")]
        disable_optimistic_cache: Option<bool>,

        #[serde(default, skip_serializing_if = "Option::is_none")]
        rewrite_ttl: Option<u32>,

        #[serde(default, skip_serializing_if = "Option::is_none")]
        timeout: Option<String>,

        #[serde(default, skip_serializing_if = "Option::is_none")]
        client_subnet: Option<String>,
    },

    Bypass {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        outbound: Option<String>,

        #[serde(flatten)]
        route_options: RouteOptionsAction,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RejectMethod {
    Default,
    Drop,
    Reply,
}
