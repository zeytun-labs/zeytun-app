use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BalancerOutbound {
    pub outbounds: Vec<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub strategy: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tolerance: Option<u16>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub delay_acceptable_ratio: Option<f64>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ttl: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_retry: Option<u32>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub weights: Option<Vec<u32>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub interrupt_exist_connections: Option<bool>,
}
