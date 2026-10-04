use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum UdpOverTcp {
    Enabled(UdpOverTcpOptions),
    Disabled(bool),
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UdpOverTcpOptions {}
