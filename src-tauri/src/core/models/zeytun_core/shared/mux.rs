use serde::{Deserialize, Serialize};

use crate::core::models::zeytun_core::shared::brutal::TcpBrutalOptions;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MultiplexOptions {
    /// Enable multiplex
    pub enabled: bool,

    /// smux | yamux | h2mux
    ///
    /// default: h2mux
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub protocol: Option<MultiplexProtocol>,

    /// Maximum connections
    ///
    /// conflicts with max_streams
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_connections: Option<u32>,

    /// Minimum multiplexed streams in a connection
    /// before opening a new connection
    ///
    /// conflicts with max_streams
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_streams: Option<u32>,

    /// Maximum multiplexed streams in a connection
    /// before opening a new connection
    ///
    /// conflicts with max_connections + min_streams
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_streams: Option<u32>,

    /// Requires zeytun-core server >= 1.3-beta9
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub padding: Option<bool>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub brutal: Option<TcpBrutalOptions>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MultiplexProtocol {
    Smux,
    Yamux,
    H2mux,
}
