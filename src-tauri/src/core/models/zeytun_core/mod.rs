use serde::{Deserialize, Serialize};

pub mod dns;
pub mod experimental;
pub mod inbound;
pub mod log;
pub mod outbound;
pub mod route;
pub mod service;
pub mod shared;

use crate::core::models::zeytun_core::{
    dns::Dns, experimental::Experimental, inbound::Inbound, log::Log, outbound::Outbound,
    route::Route, service::Service,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZeytunCoreConfig {
    pub log: Log,
    pub inbounds: Vec<Inbound>,
    pub outbounds: Vec<Outbound>,
    pub route: Route,
    pub dns: Dns,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub services: Vec<Service>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub experimental: Option<Experimental>,
}
