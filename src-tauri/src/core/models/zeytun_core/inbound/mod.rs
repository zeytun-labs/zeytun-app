use serde::{Deserialize, Serialize};

use crate::core::models::zeytun_core::inbound::{
    direct::DirectInbound, http::HttpInbound, mixed::MixedInbound, socks::SocksInbound,
    tun::TunInbound,
};

pub mod direct;
pub mod http;
pub mod mixed;
pub mod socks;
pub mod tun;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Inbound {
    pub tag: String,

    #[serde(flatten)]
    pub inbound_type: InboundType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum InboundType {
    Socks(SocksInbound),
    Http(HttpInbound),
    Mixed(MixedInbound),
    Tun(TunInbound),

    Direct(DirectInbound),
}
