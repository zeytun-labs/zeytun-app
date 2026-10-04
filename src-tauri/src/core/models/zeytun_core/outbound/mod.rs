use serde::{Deserialize, Serialize};

use crate::core::models::zeytun_core::outbound::{
    balancer::BalancerOutbound, http::HttpOutbound, hysteria2::Hysteria2Outbound,
    selector::SelectorOutbound, shadowsocks::ShadowsocksOutbound, socks::SocksOutbound,
    trojan::TrojanOutbound, tuic::TuicOutbound, urltest::UrlTestOutbound, vless::VlessOutbound,
    vmess::VmessOutbound,
};

pub mod balancer;
pub mod http;
pub mod hysteria2;
pub mod selector;
pub mod shadowsocks;
pub mod socks;
pub mod trojan;
pub mod tuic;
pub mod urltest;
pub mod vless;
pub mod vmess;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Outbound {
    pub tag: String,

    #[serde(flatten)]
    pub outbound_type: OutboundType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum OutboundType {
    Vless(VlessOutbound),
    Vmess(VmessOutbound),
    Trojan(TrojanOutbound),
    Shadowsocks(ShadowsocksOutbound),
    Hysteria2(Hysteria2Outbound),
    Tuic(TuicOutbound),
    Socks(SocksOutbound),
    Http(HttpOutbound),

    Direct(DirectOutbound),
    Block,

    Selector(SelectorOutbound),
    Urltest(UrlTestOutbound),
    Balancer(BalancerOutbound),
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DirectOutbound {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tcp_fast_open: Option<bool>,
}
