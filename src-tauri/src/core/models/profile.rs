use serde::{Deserialize, Serialize};

use crate::core::models::{
    policy::ProxyPolicy, proxy::ProxyServer, rule::RouteRule, runtime::RuntimeInboundMode,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Profile {
    // pub id: String,
    pub name: String,

    pub inbound_mode: RuntimeInboundMode,
    pub proxy_servers: Vec<ProxyServer>,
    pub proxy_policies: Vec<ProxyPolicy>,
    pub rules: Vec<RouteRule>,
    /// Prepended ahead of permanent rules / rulesets at compile (still after clash mode).
    #[serde(default)]
    pub temp_rules: Vec<RouteRule>,
    /// DTO copies for live_rules bake (ids + expires).
    #[serde(default)]
    pub live_temp: Vec<crate::core::dto::TempRule>,
    #[serde(default)]
    pub live_permanent: Vec<crate::core::dto::Rule>,
    pub rule_sets: Option<Vec<crate::core::dto::RuleSet>>,
    pub final_outbound: String,
    pub dns: Option<crate::core::dto::DnsConfig>,
    pub connection_ask: Option<crate::core::dto::ConnectionAskConfig>,
}
