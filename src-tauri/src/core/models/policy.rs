use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyPolicy {
    pub tag: String,
    pub name: String,
    pub outbounds: Vec<String>,

    #[serde(flatten)]
    pub policy_type: ProxyPolicyType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum ProxyPolicyType {
    Manual(ManualPolicy),
    Auto(AutoPolicy),
    Balancer(BalancerPolicy),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManualPolicy {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub interrupt_exist_connections: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutoPolicy {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub interval: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tolerance: Option<u32>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub idle_timeout: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub interrupt_exist_connections: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BalancerPolicy {
    /// round-robin | consistent-hashing | sticky-sessions | failover | weighted | least-connections
    pub strategy: String,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tolerance: Option<u16>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub delay_acceptable_ratio: Option<f64>,

    /// sticky-sessions cache lifetime (golang duration)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ttl: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_retry: Option<u32>,

    /// weighted: parallel to outbounds
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub weights: Option<Vec<u32>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub interrupt_exist_connections: Option<bool>,
}
