use serde::{Deserialize, Serialize};

use crate::core::models::zeytun_core::shared::{basic_auth::BasicAuth, listen::ListenOptions};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SocksInbound {
    #[serde(flatten)]
    pub listen: ListenOptions,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub users: Option<Vec<BasicAuth>>,
}
