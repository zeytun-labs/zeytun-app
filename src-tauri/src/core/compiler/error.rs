use thiserror::Error;

#[derive(Debug, Error)]
pub enum CompileError {
    #[error("missing required field `{field}` for proxy server `{proxy_server_id}`")]
    MissingField {
        proxy_server_id: String,
        field: &'static str,
    },

    #[error("unsupported protocol for proxy server `{proxy_server_id}`")]
    UnsupportedProtocol { proxy_server_id: String },

    #[error("invalid value `{value}` for field `{field}` in proxy server `{proxy_server_id}`")]
    InvalidProxyServerField {
        proxy_server_id: String,
        field: &'static str,
        value: String,
    },

    #[error("duplicate tag `{tag}`")]
    DuplicateTag { tag: String },

    #[error("no usable outbound found")]
    NoUsableOutbound,

    #[error("invalid profile: {0}")]
    InvalidProfile(String),
}
