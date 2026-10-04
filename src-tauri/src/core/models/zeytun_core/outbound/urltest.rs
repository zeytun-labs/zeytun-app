use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UrlTestOutbound {
    /// List of outbound tags to test
    pub outbounds: Vec<String>,

    /// URL used for latency test
    ///
    /// default:
    /// http://cp.cloudflare.com/
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,

    /// Test interval
    ///
    /// golang duration
    ///
    /// default: 3m
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub interval: Option<String>,

    /// Switch tolerance in milliseconds
    ///
    /// default: 50
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tolerance: Option<u16>,

    /// Idle timeout
    ///
    /// default: 30m
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub idle_timeout: Option<String>,

    /// Interrupt existing inbound connections
    /// when selected outbound changes
    ///
    /// default: false
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub interrupt_exist_connections: Option<bool>,
}
