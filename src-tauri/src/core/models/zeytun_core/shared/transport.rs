use std::collections::HashMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
// ponytail: boxed variants would change the serde wire layout of the config
// compile path; guarded by core_config_conformance, so suppress instead.
#[allow(clippy::large_enum_variant)]
pub enum V2RayTransport {
    #[serde(rename = "http")]
    Http(HttpTransport),

    #[serde(rename = "ws")]
    WebSocket(WebSocketTransport),

    #[serde(rename = "quic")]
    Quic(QuicTransport),

    #[serde(rename = "grpc")]
    Grpc(GrpcTransport),

    #[serde(rename = "httpupgrade")]
    HttpUpgrade(HttpUpgradeTransport),

    #[serde(rename = "xhttp")]
    XHttp(XHttpTransport),
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct HttpTransport {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub host: Vec<String>,

    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub path: String,

    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub method: String,

    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub headers: HashMap<String, String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub idle_timeout: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ping_timeout: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WebSocketTransport {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub path: String,

    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub headers: HashMap<String, String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_early_data: Option<u32>,

    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub early_data_header_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct QuicTransport {}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GrpcTransport {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub service_name: String,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub idle_timeout: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ping_timeout: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub permit_without_stream: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct HttpUpgradeTransport {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub host: String,

    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub path: String,

    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub headers: HashMap<String, String>,
}

/// Mirrors zeytun-core `option.V2RayXHTTPOptions` JSON shape (MVP + common advanced fields).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct XHttpTransport {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub mode: String,

    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub host: String,

    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub path: String,

    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub headers: HashMap<String, String>,

    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        rename = "xPaddingBytes"
    )]
    pub x_padding_bytes: Option<String>,

    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        rename = "noGRPCHeader"
    )]
    pub no_grpc_header: Option<bool>,

    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        rename = "noSSEHeader"
    )]
    pub no_sse_header: Option<bool>,

    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        rename = "scMaxEachPostBytes"
    )]
    pub sc_max_each_post_bytes: Option<String>,

    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        rename = "scMinPostsIntervalMs"
    )]
    pub sc_min_posts_interval_ms: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub xmux: Option<XHttpXmux>,

    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        rename = "xPaddingObfsMode"
    )]
    pub x_padding_obfs_mode: Option<bool>,

    #[serde(
        default,
        skip_serializing_if = "String::is_empty",
        rename = "xPaddingKey"
    )]
    pub x_padding_key: String,

    #[serde(
        default,
        skip_serializing_if = "String::is_empty",
        rename = "xPaddingHeader"
    )]
    pub x_padding_header: String,

    #[serde(
        default,
        skip_serializing_if = "String::is_empty",
        rename = "xPaddingPlacement"
    )]
    pub x_padding_placement: String,

    #[serde(
        default,
        skip_serializing_if = "String::is_empty",
        rename = "xPaddingMethod"
    )]
    pub x_padding_method: String,

    #[serde(
        default,
        skip_serializing_if = "String::is_empty",
        rename = "uplinkHTTPMethod"
    )]
    pub uplink_http_method: String,

    #[serde(
        default,
        skip_serializing_if = "String::is_empty",
        rename = "sessionIDPlacement"
    )]
    pub session_id_placement: String,

    #[serde(
        default,
        skip_serializing_if = "String::is_empty",
        rename = "sessionIDKey"
    )]
    pub session_id_key: String,

    #[serde(
        default,
        skip_serializing_if = "String::is_empty",
        rename = "seqPlacement"
    )]
    pub seq_placement: String,

    #[serde(default, skip_serializing_if = "String::is_empty", rename = "seqKey")]
    pub seq_key: String,

    #[serde(
        default,
        skip_serializing_if = "String::is_empty",
        rename = "uplinkDataPlacement"
    )]
    pub uplink_data_placement: String,

    #[serde(
        default,
        skip_serializing_if = "String::is_empty",
        rename = "uplinkDataKey"
    )]
    pub uplink_data_key: String,

    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        rename = "uplinkChunkSize"
    )]
    pub uplink_chunk_size: Option<String>,

    #[serde(
        default,
        skip_serializing_if = "String::is_empty",
        rename = "sessionIDTable"
    )]
    pub session_id_table: String,

    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        rename = "sessionIDLength"
    )]
    pub session_id_length: Option<String>,

    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        rename = "downloadSettings"
    )]
    pub download: Option<XHttpDownload>,
}

/// XHTTP downloadSettings → core `V2RayXHTTPDownloadOptions`.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct XHttpDownload {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub host: String,

    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub path: String,

    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub headers: HashMap<String, String>,

    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        rename = "xPaddingBytes"
    )]
    pub x_padding_bytes: Option<String>,

    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        rename = "noGRPCHeader"
    )]
    pub no_grpc_header: Option<bool>,

    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        rename = "noSSEHeader"
    )]
    pub no_sse_header: Option<bool>,

    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        rename = "scMaxEachPostBytes"
    )]
    pub sc_max_each_post_bytes: Option<String>,

    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        rename = "scMinPostsIntervalMs"
    )]
    pub sc_min_posts_interval_ms: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub xmux: Option<XHttpXmux>,

    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub server: String,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub server_port: Option<u16>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tls: Option<crate::core::models::zeytun_core::shared::tls::OutboundTlsOptions>,

    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub detour: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct XHttpXmux {
    #[serde(
        default,
        skip_serializing_if = "String::is_empty",
        rename = "maxConcurrency"
    )]
    pub max_concurrency: String,

    #[serde(
        default,
        skip_serializing_if = "String::is_empty",
        rename = "maxConnections"
    )]
    pub max_connections: String,

    #[serde(
        default,
        skip_serializing_if = "String::is_empty",
        rename = "cMaxReuseTimes"
    )]
    pub c_max_reuse_times: String,

    #[serde(
        default,
        skip_serializing_if = "String::is_empty",
        rename = "hMaxRequestTimes"
    )]
    pub h_max_request_times: String,

    #[serde(
        default,
        skip_serializing_if = "String::is_empty",
        rename = "hMaxReusableSecs"
    )]
    pub h_max_reusable_secs: String,

    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        rename = "hKeepAlivePeriod"
    )]
    pub h_keep_alive_period: Option<i64>,
}
