use std::collections::HashSet;

use crate::core::{
    dto::{ProxyPolicyType as DtoProxyPolicyType, Rule, RuleType as DtoRuleType, ZeytunCore},
    link_parser,
    models::{
        policy::{
            AutoPolicy, ManualPolicy, ProxyPolicy as ModelProxyPolicy,
            ProxyPolicyType as ModelProxyPolicyType,
        },
        profile::Profile,
        rule::{
            RejectAction, RejectMethod, RouteAction, RouteRule, RuleAction, RuleActionType,
            RuleType as ModelRuleType,
        },
        runtime::RuntimeInboundMode,
    },
};

pub struct ProfileConverter;

impl ProfileConverter {
    pub fn convert(profile: &ZeytunCore) -> Profile {
        let proxy_servers = profile
            .proxies
            .iter()
            .filter(|proxy| proxy.enabled)
            .filter_map(|proxy| link_parser::proxy_to_proxy_server(proxy).ok())
            .collect::<Vec<_>>();
        let enabled_proxy_tags = profile
            .proxies
            .iter()
            .filter(|proxy| proxy.enabled)
            .map(|proxy| proxy.tag.clone())
            .collect::<HashSet<_>>();
        let policy_tags = profile
            .policies
            .iter()
            .map(|policy| policy.tag.clone())
            .collect::<HashSet<_>>();

        let proxy_policies = profile
            .policies
            .iter()
            .map(|policy| {
                let raw_outbounds = policy.members.clone().unwrap_or_default();
                let outbounds = raw_outbounds
                    .into_iter()
                    .filter(|tag| {
                        tag != &policy.tag
                            && (enabled_proxy_tags.contains(tag) || policy_tags.contains(tag))
                    })
                    .collect::<Vec<_>>();

                let default = policy
                    .selected_member_tag
                    .as_ref()
                    .filter(|tag| outbounds.iter().any(|v| v == *tag))
                    .cloned()
                    .or_else(|| outbounds.first().cloned());

                let kind = policy
                    .kind
                    .as_ref()
                    .unwrap_or(&DtoProxyPolicyType::Selector);

                let policy_type = match kind {
                    DtoProxyPolicyType::Selector => ModelProxyPolicyType::Manual(ManualPolicy {
                        default,
                        interrupt_exist_connections: Some(true),
                    }),
                    DtoProxyPolicyType::Urltest => ModelProxyPolicyType::Auto(AutoPolicy {
                        url: Some(
                            policy
                                .test_url
                                .clone()
                                .unwrap_or_else(|| "http://cp.cloudflare.com/".to_string()),
                        ),
                        interval: policy.interval_seconds.map(|seconds| format!("{seconds}s")),
                        tolerance: policy.tolerance_ms,
                        idle_timeout: None,
                        interrupt_exist_connections: Some(true),
                    }),
                    DtoProxyPolicyType::Balancer => ModelProxyPolicyType::Balancer(
                        crate::core::models::policy::BalancerPolicy {
                            strategy: policy
                                .strategy
                                .clone()
                                .unwrap_or_else(|| "round-robin".to_string()),
                            tolerance: policy.tolerance_ms.map(|t| t as u16),
                            delay_acceptable_ratio: None,
                            ttl: None,
                            max_retry: None,
                            weights: policy.weights.clone(),
                            interrupt_exist_connections: Some(true),
                        },
                    ),
                };

                ModelProxyPolicy {
                    tag: policy.tag.clone(),
                    name: policy.name.clone(),
                    outbounds,
                    policy_type,
                }
            })
            .collect::<Vec<_>>();

        let rules = profile
            .rules
            .iter()
            .filter(|rule| !matches!(rule.kind, DtoRuleType::Final))
            .map(Self::core_rule_to_model)
            .collect::<Vec<_>>();

        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0);
        let live_temp: Vec<_> = profile
            .temp_rules
            .iter()
            .filter(|r| r.enabled && !r.is_expired(now_ms) && !matches!(r.kind, DtoRuleType::Final))
            .cloned()
            .collect();
        let temp_rules = live_temp
            .iter()
            .map(|r| Self::core_rule_to_model(&r.to_rule()))
            .collect::<Vec<_>>();
        let live_permanent: Vec<_> = profile
            .rules
            .iter()
            .filter(|r| r.enabled && !matches!(r.kind, DtoRuleType::Final))
            .cloned()
            .collect();

        let mixed = crate::core::models::runtime::RuntimeMixedInbound {
            listen: profile.local_proxy.listen.clone(),
            listen_port: profile.local_proxy.mixed_port.unwrap_or(6060),
        };

        let tun = match &profile.local_proxy.mode {
            Some(crate::core::dto::InboundMode::Tun) => {
                Some(crate::core::models::runtime::RuntimeTunInbound {
                    interface_name: profile
                        .local_proxy
                        .tun_name
                        .clone()
                        .unwrap_or_else(|| "utun9".to_string()),
                    mtu: profile.local_proxy.tun_mtu.unwrap_or(9000),
                    auto_route: profile.local_proxy.tun_auto_route.unwrap_or(true),
                })
            }
            _ => None,
        };

        let inbound_mode = RuntimeInboundMode {
            mixed,
            tun,
            log_level: profile.local_proxy.log_level.clone(),
        };

        Profile {
            name: "Default".to_string(),
            inbound_mode,
            proxy_servers,
            proxy_policies,
            rules,
            temp_rules,
            live_temp,
            live_permanent,
            rule_sets: profile.rule_sets.clone(),
            final_outbound: profile.final_outbound(),
            dns: profile.dns.clone(),
            connection_ask: profile.local_proxy.connection_ask.clone(),
        }
    }

    pub fn core_rule_to_model_pub(rule: &Rule) -> RouteRule {
        Self::core_rule_to_model(rule)
    }

    fn core_rule_to_model(rule: &Rule) -> RouteRule {
        let rule_type = match rule.kind {
            DtoRuleType::Final => unreachable!("FINAL is handled as route.final"),
            DtoRuleType::Domain => ModelRuleType::Domain,
            DtoRuleType::DomainSuffix => ModelRuleType::DomainSuffix,
            DtoRuleType::DomainKeyword => ModelRuleType::DomainKeyword,
            DtoRuleType::DomainRegex => ModelRuleType::DomainRegex,
            DtoRuleType::IpCidr => ModelRuleType::IpCidr,
            DtoRuleType::InPort => ModelRuleType::InPort,
            DtoRuleType::DestPort => ModelRuleType::DestPort,
            DtoRuleType::Geoip => ModelRuleType::GeoIp,
            DtoRuleType::Geosite => ModelRuleType::GeoSite,
            DtoRuleType::ProcessName => ModelRuleType::ProcessName,
            DtoRuleType::ProcessPath => ModelRuleType::ProcessPath,
            DtoRuleType::ProcessPathRegex => ModelRuleType::ProcessPathRegex,
            DtoRuleType::Protocol => ModelRuleType::Protocol,
        };

        let action = if rule.outbound.eq_ignore_ascii_case("reject") {
            RuleActionType::Reject(RejectAction {
                method: Some(RejectMethod::Default),
                no_drop: None,
            })
        } else if rule.outbound.eq_ignore_ascii_case("hijack-dns") {
            RuleActionType::HijackDns
        } else if rule.outbound.eq_ignore_ascii_case("bypass") {
            RuleActionType::Bypass
        } else {
            RuleActionType::Route(RouteAction {
                outbound: rule.outbound.clone(),
            })
        };

        RouteRule {
            rule_type,
            value: rule.value.clone(),
            action: RuleAction { action },
            used: 0,
            comment: (!rule.comment.trim().is_empty()).then_some(rule.comment.clone()),
        }
    }
}
