use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::core::models::zeytun_core::shared::{dial::DialOptions, tls::OutboundTlsOptions};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpOutbound {
    pub server: String,
    pub server_port: u16,

    #[serde(flatten)]
    pub dial: DialOptions,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub password: Option<String>,

    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub headers: HashMap<String, String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub tls: Option<OutboundTlsOptions>,
}
