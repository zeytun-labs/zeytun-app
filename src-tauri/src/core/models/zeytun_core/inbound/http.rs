use serde::{Deserialize, Serialize};

use crate::core::models::zeytun_core::shared::{
    basic_auth::BasicAuth, listen::ListenOptions, tls::InboundTlsOptions,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpInbound {
    #[serde(flatten)]
    pub listen: ListenOptions,

    /// SOCKS + HTTP users
    ///
    /// no authentication if empty
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub users: Option<Vec<BasicAuth>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub tls: Option<InboundTlsOptions>,

    /// Automatically set system proxy
    ///
    /// Linux / Android / Windows / macOS only
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub set_system_proxy: Option<bool>,
}
