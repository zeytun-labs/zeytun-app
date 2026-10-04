use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SelectorOutbound {
    /// List of outbound tags to select from
    pub outbounds: Vec<String>,

    /// Default selected outbound tag
    ///
    /// if empty => first outbound used
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default: Option<String>,

    /// Interrupt existing inbound connections
    /// when selected outbound changes
    ///
    /// default: false
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub interrupt_exist_connections: Option<bool>,
}
