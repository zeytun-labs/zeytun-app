use serde::{Deserialize, Serialize};

use crate::core::models::zeytun_core::shared::{listen::ListenOptions, network::NetworkOptions};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DirectInbound {
    #[serde(flatten)]
    pub listen: ListenOptions,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub network: Option<NetworkOptions>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub override_address: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub override_port: Option<u16>,
}
