use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Http2Options {
    /// Idle connection timeout
    ///
    /// golang duration format
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub idle_timeout: Option<String>,

    /// Keep alive period
    ///
    /// golang duration format
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub keep_alive_period: Option<String>,

    /// HTTP2 stream-level flow-control receive window
    ///
    /// e.g. "64 MB"
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stream_receive_window: Option<String>,

    /// HTTP2 connection-level flow-control receive window
    ///
    /// e.g. "64 MB"
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub connection_receive_window: Option<String>,

    /// Maximum concurrent streams per connection
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_concurrent_streams: Option<u32>,
}
