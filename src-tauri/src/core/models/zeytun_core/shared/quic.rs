use serde::{Deserialize, Serialize};

use crate::core::models::zeytun_core::shared::http2::Http2Options;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct QuicOptions {
    /// Initial QUIC packet size
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub initial_packet_size: Option<u16>,

    /// Disable QUIC path MTU discovery
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disable_path_mtu_discovery: Option<bool>,

    /// QUIC fields also include HTTP2 fields
    #[serde(flatten)]
    pub http2: Http2Options,
}
