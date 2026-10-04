use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::core::models::zeytun_core::route::{
    ClashMode, ClientNetworkType, IpVersion, LogicalMode, LogicalType,
};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Dns {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub servers: Option<Vec<DnsServer>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rules: Option<Vec<DnsRule>>,

    #[serde(default, rename = "final", skip_serializing_if = "Option::is_none")]
    pub final_server: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub strategy: Option<DnsDomainStrategy>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disable_cache: Option<bool>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disable_expire: Option<bool>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_capacity: Option<u32>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub optimistic: Option<DnsOptimistic>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reverse_mapping: Option<bool>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub client_subnet: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DnsServer {
    #[serde(rename = "type")]
    pub server_type: DnsServerType,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tag: Option<String>,

    #[serde(flatten)]
    pub options: HashMap<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DnsServerType {
    Local,
    Hosts,
    Tcp,
    Udp,
    Tls,
    Quic,
    Https,
    Http3,
    Dhcp,
    Mdns,
    Fakeip,
    Tailscale,
    Resolved,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum DnsOptimistic {
    Enabled(bool),
    Options(DnsOptimisticOptions),
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DnsOptimisticOptions {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DnsDomainStrategy {
    PreferIpv4,
    PreferIpv6,
    Ipv4Only,
    Ipv6Only,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
// ponytail: boxed variants would change the serde wire layout of the config
// compile path; guarded by core_config_conformance, so suppress instead.
#[allow(clippy::large_enum_variant)]
pub enum DnsRule {
    Default(DefaultDnsRule),
    Logical(LogicalDnsRule),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DefaultDnsRule {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inbound: Option<Vec<String>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ip_version: Option<IpVersion>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub query_type: Option<Vec<DnsQueryType>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub network: Option<Vec<DnsNetwork>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auth_user: Option<Vec<String>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub protocol: Option<Vec<String>>,

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
    pub user_id: Option<Vec<i32>>,

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
    pub source_mac_address: Option<Vec<String>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_hostname: Option<Vec<String>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preferred_by: Option<Vec<String>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wifi_ssid: Option<Vec<String>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wifi_bssid: Option<Vec<String>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rule_set: Option<Vec<String>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rule_set_ip_cidr_match_source: Option<bool>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub match_response: Option<bool>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ip_accept_any: Option<bool>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub response_rcode: Option<DnsResponseCode>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub response_answer: Option<Vec<String>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub response_ns: Option<Vec<String>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub response_extra: Option<Vec<String>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub invert: Option<bool>,

    #[serde(flatten)]
    pub action: DnsRuleAction,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogicalDnsRule {
    #[serde(rename = "type")]
    pub rule_type: LogicalType,

    pub mode: LogicalMode,

    pub rules: Vec<DnsRule>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub invert: Option<bool>,

    #[serde(flatten)]
    pub action: DnsRuleAction,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum DnsQueryType {
    Number(u16),
    Name(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DnsNetwork {
    Tcp,
    Udp,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum DnsResponseCode {
    Noerror,
    Formerr,
    Servfail,
    Nxdomain,
    Notimp,
    Refused,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "kebab-case")]
pub enum DnsRuleAction {
    Route {
        server: String,

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

    Evaluate {
        server: String,

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

    Respond,

    RouteOptions {
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

    Reject {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        method: Option<DnsRejectMethod>,

        #[serde(default, skip_serializing_if = "Option::is_none")]
        no_drop: Option<bool>,
    },

    Predefined {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        rcode: Option<DnsResponseCode>,

        #[serde(default, skip_serializing_if = "Option::is_none")]
        answer: Option<Vec<String>>,

        #[serde(default, skip_serializing_if = "Option::is_none")]
        ns: Option<Vec<String>>,

        #[serde(default, skip_serializing_if = "Option::is_none")]
        extra: Option<Vec<String>>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DnsRejectMethod {
    Default,
    Drop,
}
