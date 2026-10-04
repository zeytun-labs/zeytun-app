use crate::core::{
    compiler::warning::CompileWarning,
    models::{
        rule::RouteRule,
        zeytun_core::{log::Log, outbound::Outbound},
    },
};

#[derive(Debug, Clone)]
pub struct RuntimeProfile {
    pub name: String,

    pub log: Log,
    pub tags: Vec<String>,
    pub inbound_mode: RuntimeInboundMode,
    pub outbounds: Vec<RuntimeOutbound>,
    pub policies: Vec<RuntimePolicy>,
    pub rules: Vec<RouteRule>,
    /// System-static only uses rules/temp_rules for compile validation; live overlay uses these.
    pub live_temp: Vec<crate::core::dto::TempRule>,
    pub live_permanent: Vec<crate::core::dto::Rule>,
    pub rule_sets: Option<Vec<crate::core::dto::RuleSet>>,
    pub final_outbound: String,
    pub dns: Option<crate::core::dto::DnsConfig>,
    pub connection_ask: Option<crate::core::dto::ConnectionAskConfig>,

    pub ignored_proxy_servers: Vec<IgnoredProxyServer>,
    pub warnings: Vec<CompileWarning>,
}

#[derive(Debug, Clone)]
pub struct RuntimeOutbound {
    pub tag: String,
    pub source_proxy_server_id: String,
    pub outbound: Outbound,
}

#[derive(Debug, Clone)]
pub struct RuntimePolicy {
    pub tag: String,
    pub source_proxy_policy_id: String,
    pub outbounds: Vec<String>,
    pub policy: Outbound,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RuntimeInboundMode {
    pub mixed: RuntimeMixedInbound,
    pub tun: Option<RuntimeTunInbound>,
    pub log_level: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RuntimeMixedInbound {
    pub listen: String,
    pub listen_port: u16,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RuntimeTunInbound {
    pub interface_name: String,
    pub mtu: u32,
    pub auto_route: bool,
}

impl Default for RuntimeInboundMode {
    fn default() -> Self {
        Self {
            mixed: RuntimeMixedInbound {
                listen: "127.0.0.1".to_string(),
                listen_port: 6061,
            },
            tun: None,
            log_level: Some("info".to_string()),
        }
    }
}

#[derive(Debug, Clone)]
pub struct IgnoredProxyServer {
    pub tag: String,
    pub name: String,
    pub reason: String,
}
