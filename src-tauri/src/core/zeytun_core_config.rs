use serde_json::Value;

use crate::core::{
    dto::{OutboundMode, DEFAULT_SELECTOR_POLICY_TAG},
    models::{
        rule::{
            RejectMethod as ModelRejectMethod, RouteRule as ModelRouteRule, RuleActionType,
            RuleType,
        },
        runtime::{RuntimeInboundMode, RuntimePolicy, RuntimeProfile},
        zeytun_core::{
            dns::Dns,
            experimental::{CacheFile, ClashApi, Experimental, V2RayApi, V2RayApiStats},
            inbound::{mixed::MixedInbound, tun::TunInbound, Inbound, InboundType},
            outbound::{Outbound, OutboundType},
            route::{
                ClashMode, DefaultRouteRule, HttpClientRef, LocalRouteRuleSet,
                LocalRouteRuleSetType, RejectMethod as ZeytunCoreRejectMethod, RemoteRouteRuleSet,
                RemoteRouteRuleSetType, Route, RouteOptionsAction,
                RouteRule as ZeytunCoreRouteRule, RouteRuleSet, RuleAction, RuleSetFormat,
            },
            service::Service,
            shared::listen::ListenOptions,
            ZeytunCoreConfig,
        },
    },
};

pub const SECRET: &str = "SECRET";
pub const DNS_HOSTS_SERVER_TAG: &str = "local-hosts";
pub const DNS_LOCAL_SERVER_TAG: &str = "local-dns";
pub const DNS_FOLLOW_MODE: &str = "follow-mode";
pub fn dns_direct_tag(tag: &str) -> String {
    format!("__dns-direct-{tag}")
}

pub struct ZeytunCoreConfigBuilder {
    include_clash_api: bool,
    include_v2ray_api: bool,
}

impl ZeytunCoreConfigBuilder {
    pub fn new(include_clash_api: bool, include_v2ray_api: bool) -> Self {
        Self {
            include_clash_api,
            include_v2ray_api,
        }
    }

    pub fn clash_api_enabled(&self) -> bool {
        self.include_clash_api
    }

    pub fn build(
        &self,
        profile: &RuntimeProfile,
        outbound_mode: &OutboundMode,
    ) -> Result<Value, String> {
        let inbounds = self.build_inbounds(&profile.inbound_mode);
        let outbounds = self.build_outbounds(profile);
        let experimental = self.build_experimental(profile, outbound_mode, &inbounds, &outbounds);

        let services = vec![Service {
            service_type: "api".to_string(),
            listen: Some("127.0.0.1".to_string()),
            listen_port: Some(crate::core::constants::GRPC_API_PORT),
            secret: Some("ZEYTUN-API-SECRET".to_string()),
        }];

        let config = ZeytunCoreConfig {
            log: profile.log.clone(),
            inbounds,
            outbounds,
            route: self.build_route(profile, outbound_mode),
            dns: self.build_dns(profile)?,
            services,
            experimental: Some(experimental),
        };

        let mut config = serde_json::to_value(config)
            .map_err(|e| format!("failed to serialize zeytun-core config: {e}"))?;
        {
            // Resolving the proxy endpoint through that proxy creates a cycle.
            // Only first-hop endpoint hostnames bootstrap locally. Chained
            // endpoints keep remote resolution through their explicit detour.
            for outbound in config["outbounds"].as_array_mut().into_iter().flatten() {
                let endpoint_is_domain = outbound["server"]
                    .as_str()
                    .is_some_and(|server| server.parse::<std::net::IpAddr>().is_err());
                let first_hop = outbound["detour"]
                    .as_str()
                    .is_none_or(|tag| tag == "direct");
                if endpoint_is_domain && first_hop && outbound.get("domain_resolver").is_none() {
                    outbound["domain_resolver"] = serde_json::json!(DNS_LOCAL_SERVER_TAG);
                }
            }
        }
        Ok(config)
    }

    fn global_outbound(profile: &RuntimeProfile) -> String {
        profile
            .policies
            .iter()
            .find(|policy| policy.source_proxy_policy_id == DEFAULT_SELECTOR_POLICY_TAG)
            .or_else(|| profile.policies.first())
            .map(|policy| policy.tag.clone())
            .or_else(|| {
                profile
                    .outbounds
                    .first()
                    .map(|outbound| outbound.tag.clone())
            })
            .unwrap_or_else(|| "direct".into())
    }

    fn build_dns(&self, profile: &RuntimeProfile) -> Result<Dns, String> {
        use crate::core::models::zeytun_core::dns::{
            DnsRule, DnsRuleAction, DnsServer, DnsServerType,
        };

        let mut dns = Dns::default();

        let mut profile_dns = profile
            .dns
            .clone()
            .unwrap_or_else(crate::core::dto::DnsConfig::with_default_server);
        if profile_dns.mode.is_some() {
            profile_dns.migrate_legacy();
        }
        let follows_mode = |tag: &str| {
            profile_dns
                .servers
                .as_deref()
                .unwrap_or_default()
                .iter()
                .any(|server| {
                    server.tag == tag && server.detour.as_deref() == Some(DNS_FOLLOW_MODE)
                })
        };
        let proxy = Self::global_outbound(profile);
        let usable_proxy = profile
            .outbounds
            .iter()
            .any(|outbound| !matches!(outbound.outbound.outbound_type, OutboundType::Block));
        let proxy = if proxy == "direct" || !usable_proxy {
            "block".to_string()
        } else {
            proxy
        };
        {
            let mut servers = Vec::new();
            let mut rules = Vec::new();

            let mut grouped_hosts: Vec<(String, Vec<String>)> = Vec::new();
            for host in profile_dns
                .hosts
                .as_deref()
                .unwrap_or_default()
                .iter()
                .filter(|host| host.enabled)
            {
                if let Some((_, addresses)) = grouped_hosts
                    .iter_mut()
                    .find(|(domain, _)| domain == &host.domain)
                {
                    addresses.push(host.address.clone());
                } else {
                    grouped_hosts.push((host.domain.clone(), vec![host.address.clone()]));
                }
            }

            if !grouped_hosts.is_empty() {
                let domains = grouped_hosts
                    .iter()
                    .map(|(domain, _)| domain.clone())
                    .collect::<Vec<_>>();
                let predefined = grouped_hosts
                    .into_iter()
                    .map(|(domain, addresses)| {
                        (
                            domain,
                            serde_json::Value::Array(
                                addresses
                                    .into_iter()
                                    .map(serde_json::Value::String)
                                    .collect(),
                            ),
                        )
                    })
                    .collect::<serde_json::Map<_, _>>();
                let mut options = std::collections::HashMap::new();
                options.insert(
                    "predefined".to_string(),
                    serde_json::Value::Object(predefined),
                );
                servers.push(DnsServer {
                    server_type: DnsServerType::Hosts,
                    tag: Some(DNS_HOSTS_SERVER_TAG.to_string()),
                    options,
                });

                let mut host_rule = Self::empty_dns_rule(DnsRuleAction::Route {
                    server: DNS_HOSTS_SERVER_TAG.to_string(),
                    disable_cache: None,
                    disable_optimistic_cache: None,
                    rewrite_ttl: None,
                    timeout: None,
                    client_subnet: None,
                });
                host_rule.domain = Some(domains);
                rules.push(DnsRule::Default(host_rule));
            }

            // Direct hostname endpoints use a real, named bootstrap resolver.
            // Proxy detours resolve upstream hostnames remotely and must not be
            // forced through the machine's potentially filtered DNS.
            use crate::core::dns_config::{DnsEndpoint, DnsTransport};
            let bootstrap = profile_dns
                .servers
                .as_deref()
                .unwrap_or_default()
                .iter()
                .find_map(|s| {
                    if s.detour.as_deref().is_some_and(|d| d != "direct") {
                        return None;
                    }
                    DnsEndpoint::parse(&s.address)
                        .ok()
                        .filter(|e| e.host.parse::<std::net::IpAddr>().is_ok())
                        .map(|_| s.tag.clone())
                })
                .unwrap_or_else(|| DNS_LOCAL_SERVER_TAG.to_string());
            if let Some(custom_servers) = &profile_dns.servers {
                for server in custom_servers {
                    let endpoint = DnsEndpoint::parse(&server.address)?;
                    let mut options = std::collections::HashMap::new();
                    options.insert("server".to_string(), Value::String(endpoint.host.clone()));
                    if let Some(port) = endpoint.port {
                        options.insert("server_port".to_string(), serde_json::json!(port));
                    }
                    if let Some(path) = endpoint.path {
                        options.insert("path".to_string(), Value::String(path));
                    }
                    let follow_mode = server.detour.as_deref() == Some(DNS_FOLLOW_MODE);
                    let detour = if follow_mode {
                        Some(proxy.as_str())
                    } else {
                        server
                            .detour
                            .as_deref()
                            .filter(|d| !d.is_empty() && !d.eq_ignore_ascii_case("direct"))
                    };
                    if let Some(detour) = detour {
                        options.insert("detour".to_string(), Value::String(detour.to_string()));
                    }
                    let hostname = endpoint.host.parse::<std::net::IpAddr>().is_err();
                    if hostname && detour.is_none() {
                        options.insert(
                            "domain_resolver".to_string(),
                            Value::String(bootstrap.clone()),
                        );
                    }
                    let server_type = match endpoint.transport {
                        DnsTransport::Udp => DnsServerType::Udp,
                        DnsTransport::Tcp => DnsServerType::Tcp,
                        DnsTransport::Tls => DnsServerType::Tls,
                        DnsTransport::Https => DnsServerType::Https,
                        DnsTransport::Quic => DnsServerType::Quic,
                    };
                    if follow_mode {
                        let mut direct_options = options.clone();
                        direct_options.remove("detour");
                        if hostname {
                            direct_options.insert(
                                "domain_resolver".to_string(),
                                Value::String(bootstrap.clone()),
                            );
                        }
                        servers.push(DnsServer {
                            server_type: server_type.clone(),
                            tag: Some(dns_direct_tag(&server.tag)),
                            options: direct_options,
                        });
                    }
                    servers.push(DnsServer {
                        server_type,
                        tag: Some(server.tag.clone()),
                        options,
                    });
                }
            }

            if let Some(true) = profile_dns.fake_ip {
                let mut options = std::collections::HashMap::new();
                options.insert(
                    "inet4_range".to_string(),
                    serde_json::Value::String("198.18.0.0/15".to_string()),
                );
                servers.push(DnsServer {
                    server_type: DnsServerType::Fakeip,
                    tag: Some("fakeip".to_string()),
                    options,
                });
            }

            if let Some(profile_rules) = &profile_dns.rules {
                for rule in profile_rules.iter().filter(|rule| rule.enabled) {
                    let action = if rule.target == "block" {
                        DnsRuleAction::Reject {
                            method: None,
                            no_drop: None,
                        }
                    } else {
                        DnsRuleAction::Route {
                            server: rule.target.clone(),
                            disable_cache: None,
                            disable_optimistic_cache: None,
                            rewrite_ttl: None,
                            timeout: None,
                            client_subnet: None,
                        }
                    };
                    let mut default = Self::empty_dns_rule(action);
                    match rule.kind.as_str() {
                        "domain" => default.domain = Some(vec![rule.value.clone()]),
                        "domain_suffix" => {
                            default.domain_suffix = Some(vec![rule.value.clone()]);
                        }
                        "ruleset" => default.rule_set = Some(vec![rule.value.clone()]),
                        _ => continue,
                    }
                    if follows_mode(&rule.target) {
                        let mut direct = default.clone();
                        direct.clash_mode = Some(ClashMode::Direct);
                        direct.action = DnsRuleAction::Route {
                            server: dns_direct_tag(&rule.target),
                            disable_cache: None,
                            disable_optimistic_cache: None,
                            rewrite_ttl: None,
                            timeout: None,
                            client_subnet: None,
                        };
                        rules.push(DnsRule::Default(direct));
                    }
                    rules.push(DnsRule::Default(default));
                }
            }

            // Final is always an authored resolver choice, never a hidden
            // provider. Every profile gets the same local-name exemptions.
            let requested_final = profile_dns
                .final_server
                .as_deref()
                .map(str::trim)
                .filter(|tag| !tag.is_empty())
                .unwrap_or(DNS_LOCAL_SERVER_TAG);
            let final_tag = if requested_final == DNS_LOCAL_SERVER_TAG
                || servers.iter().any(|server| {
                    server.tag.as_deref() == Some(requested_final)
                        && !matches!(
                            server.server_type,
                            DnsServerType::Hosts | DnsServerType::Fakeip
                        )
                }) {
                requested_final.to_string()
            } else {
                return Err("Final DNS resolver is not a configured DNS server".into());
            };
            servers.push(DnsServer {
                server_type: DnsServerType::Local,
                tag: Some(DNS_LOCAL_SERVER_TAG.into()),
                options: std::collections::HashMap::new(),
            });
            let mut local = Self::dns_route_rule(DNS_LOCAL_SERVER_TAG);
            local.domain_suffix = Some(vec!["local".into(), "home.arpa".into()]);
            rules.push(DnsRule::Default(local));
            let mut single_label = Self::dns_route_rule(DNS_LOCAL_SERVER_TAG);
            single_label.domain_regex = Some(vec![r"^[^.]+\.?$".into()]);
            rules.push(DnsRule::Default(single_label));
            // Direct mode resolves real addresses before FakeIP. Explicit
            // server detours remain explicit, even when traffic mode is Direct.
            let direct_final = if follows_mode(&final_tag) {
                dns_direct_tag(&final_tag)
            } else {
                final_tag.clone()
            };
            let mut direct = Self::dns_route_rule(&direct_final);
            direct.clash_mode = Some(ClashMode::Direct);
            rules.push(DnsRule::Default(direct));
            if profile_dns.fake_ip == Some(true) {
                use crate::core::models::zeytun_core::dns::DnsQueryType;
                let mut fake = Self::empty_dns_rule(DnsRuleAction::Route {
                    server: "fakeip".to_string(),
                    disable_cache: None,
                    disable_optimistic_cache: None,
                    rewrite_ttl: None,
                    timeout: None,
                    client_subnet: None,
                });
                fake.query_type = Some(vec![
                    DnsQueryType::Name("A".into()),
                    DnsQueryType::Name("AAAA".into()),
                ]);
                rules.push(DnsRule::Default(fake));
            }

            if !servers.is_empty() {
                dns.servers = Some(servers);
            }

            if !rules.is_empty() {
                dns.rules = Some(rules);
            }
            dns.final_server = Some(final_tag);
        }

        if dns.servers.is_none() {
            dns.servers = Some(vec![DnsServer {
                server_type: DnsServerType::Local,
                tag: Some(DNS_LOCAL_SERVER_TAG.to_string()),
                options: std::collections::HashMap::new(),
            }]);
            dns.final_server = Some(DNS_LOCAL_SERVER_TAG.to_string());
        }
        dns.reverse_mapping = Some(true);
        Ok(dns)
    }

    fn dns_route_rule(server: &str) -> crate::core::models::zeytun_core::dns::DefaultDnsRule {
        Self::empty_dns_rule(
            crate::core::models::zeytun_core::dns::DnsRuleAction::Route {
                server: server.into(),
                disable_cache: None,
                disable_optimistic_cache: None,
                rewrite_ttl: None,
                timeout: None,
                client_subnet: None,
            },
        )
    }

    fn empty_dns_rule(
        action: crate::core::models::zeytun_core::dns::DnsRuleAction,
    ) -> crate::core::models::zeytun_core::dns::DefaultDnsRule {
        crate::core::models::zeytun_core::dns::DefaultDnsRule {
            inbound: None,
            ip_version: None,
            query_type: None,
            network: None,
            auth_user: None,
            protocol: None,
            domain: None,
            domain_suffix: None,
            domain_keyword: None,
            domain_regex: None,
            source_ip_cidr: None,
            source_ip_is_private: None,
            source_port: None,
            source_port_range: None,
            port: None,
            port_range: None,
            process_name: None,
            process_path: None,
            process_path_regex: None,
            package_name: None,
            package_name_regex: None,
            user: None,
            user_id: None,
            clash_mode: None,
            network_type: None,
            network_is_expensive: None,
            network_is_constrained: None,
            interface_address: None,
            network_interface_address: None,
            default_interface_address: None,
            source_mac_address: None,
            source_hostname: None,
            preferred_by: None,
            wifi_ssid: None,
            wifi_bssid: None,
            rule_set: None,
            rule_set_ip_cidr_match_source: None,
            match_response: None,
            ip_accept_any: None,
            response_rcode: None,
            response_answer: None,
            response_ns: None,
            response_extra: None,
            invert: None,
            action,
        }
    }

    fn build_experimental(
        &self,
        profile: &RuntimeProfile,
        outbound_mode: &OutboundMode,
        inbounds: &[Inbound],
        outbounds: &[Outbound],
    ) -> Experimental {
        let inbound_tags = inbounds.iter().map(|inbound| inbound.tag.clone()).collect();
        let outbound_tags = outbounds
            .iter()
            .map(|outbound| outbound.tag.clone())
            .collect();

        let mode_str = serde_json::to_value(outbound_mode)
            .ok()
            .and_then(|v| v.as_str().map(|s| s.to_string()))
            .unwrap_or_else(|| "rule".to_string());

        let clash_api = self.include_clash_api.then(|| ClashApi {
            external_controller: Some(self.default_clash_controller(&profile.inbound_mode)),
            external_ui: None,
            external_ui_download_url: None,
            external_ui_download_detour: Some("direct".to_string()),
            secret: Some(SECRET.to_string()),
            default_mode: Some(mode_str),
            access_control_allow_origin: Some(vec!["*".to_string()]),
            access_control_allow_private_network: Some(true),
        });

        let v2ray_api = self.include_v2ray_api.then(|| V2RayApi {
            listen: Some("127.0.0.1:10085".to_string()),
            stats: Some(V2RayApiStats {
                enabled: Some(true),
                inbounds: Some(inbound_tags),
                outbounds: Some(outbound_tags),
                users: Some(Vec::new()),
            }),
        });

        Experimental {
            // Persist remote rule-set bytes so a later offline/DNS-fail boot can restore.
            cache_file: Some(CacheFile {
                enabled: Some(true),
                path: Some("cache.db".into()),
                store_fakeip: profile
                    .dns
                    .as_ref()
                    .and_then(|dns| dns.fake_ip)
                    .unwrap_or(false)
                    .then_some(true),
                store_dns: Some(true),
                ..Default::default()
            }),
            clash_api,
            v2ray_api,
        }
    }

    pub fn default_clash_controller(&self, inbound_mode: &RuntimeInboundMode) -> String {
        let listen = &inbound_mode.mixed.listen;
        if !listen.is_empty() {
            format!("{}:{}", listen, crate::core::constants::CLASH_API_PORT)
        } else {
            format!(
                "{}:{}",
                crate::core::constants::LOCALHOST,
                crate::core::constants::CLASH_API_PORT
            )
        }
    }

    fn build_inbounds(&self, inbound_mode: &RuntimeInboundMode) -> Vec<Inbound> {
        let mut inbounds = Vec::new();

        inbounds.push(Inbound {
            tag: "mixed-in".to_string(),
            inbound_type: InboundType::Mixed(MixedInbound {
                listen: ListenOptions {
                    listen: Some(inbound_mode.mixed.listen.clone()),
                    listen_port: Some(inbound_mode.mixed.listen_port),
                    ..Default::default()
                },
                users: None,
                set_system_proxy: None,
            }),
        });

        if let Some(tun) = &inbound_mode.tun {
            inbounds.push(Inbound {
                tag: "tun-in".to_string(),
                inbound_type: InboundType::Tun(TunInbound {
                    listen: ListenOptions::default(),
                    interface_name: (!tun.interface_name.is_empty())
                        .then_some(tun.interface_name.clone()),
                    address: vec![
                        "172.19.0.1/30".to_string(),
                        "fdfe:dcba:9876::1/126".to_string(),
                    ],
                    mtu: Some(tun.mtu),
                    stack: None,
                    dns_mode: None,
                    auto_route: Some(tun.auto_route),
                    strict_route: None,
                    auto_redirect: None,
                    auto_redirect_input_mark: None,
                    auto_redirect_output_mark: None,
                    auto_redirect_reset_mark: None,
                    auto_redirect_nfqueue: None,
                    auto_redirect_iproute2_fallback_rule_index: None,
                    exclude_mptcp: None,
                    loopback_address: None,
                    route_address: None,
                    route_exclude_address: None,
                    route_address_set: None,
                    route_exclude_address_set: None,
                    endpoint_independent_nat: None,
                    udp_timeout: None,
                    include_interface: None,
                    exclude_interface: None,
                    include_uid: None,
                    include_uid_range: None,
                    exclude_uid: None,
                    exclude_uid_range: None,
                    include_android_user: None,
                    include_package: None,
                    exclude_package: None,
                    include_mac_address: None,
                    exclude_mac_address: None,
                    platform: None,
                }),
            });
        }

        inbounds
    }

    fn build_outbounds(&self, profile: &RuntimeProfile) -> Vec<Outbound> {
        let mut outbounds =
            Vec::with_capacity(profile.outbounds.len() + profile.policies.len() + 2);

        for policy in &profile.policies {
            outbounds.push(self.build_policy_outbound(policy));
        }

        for outbound in &profile.outbounds {
            outbounds.push(outbound.outbound.clone());
        }

        outbounds.push(Outbound {
            tag: "direct".to_string(),
            outbound_type: OutboundType::Direct(
                crate::core::models::zeytun_core::outbound::DirectOutbound {
                    tcp_fast_open: Some(true),
                },
            ),
        });

        outbounds.push(Outbound {
            tag: "block".to_string(),
            outbound_type: OutboundType::Block,
        });

        outbounds
    }

    fn build_policy_outbound(&self, policy: &RuntimePolicy) -> Outbound {
        let mut outbound = policy.policy.clone();

        match &mut outbound.outbound_type {
            OutboundType::Selector(selector) => {
                selector.outbounds = policy.outbounds.clone();
                if selector.default.is_none() {
                    selector.default = policy.outbounds.first().cloned();
                }
            }
            OutboundType::Urltest(urltest) => {
                urltest.outbounds = policy.outbounds.clone();
            }
            OutboundType::Balancer(bal) => {
                bal.outbounds = policy.outbounds.clone();
            }
            _ => {}
        }

        outbound
    }

    fn build_route(&self, profile: &RuntimeProfile, outbound_mode: &OutboundMode) -> Route {
        let global_outbound = Self::global_outbound(profile);

        let mut rules = self.build_clash_mode_rules(&global_outbound);

        // System-static only: clash mode + enabled rulesets. User rules live in route.live_rules.
        if let Some(rule_sets) = &profile.rule_sets {
            for rs in rule_sets.iter().filter(|rs| rs.enabled) {
                rules.push(ZeytunCoreRouteRule::Default(DefaultRouteRule {
                    rule_set: Some(vec![rs.tag.clone()]),
                    action: RuleAction::Route {
                        outbound: rs.action.clone(),
                        route_options: RouteOptionsAction::default(),
                    },
                    ..Self::empty_route_rule()
                }));
            }
        }

        // User permanent/temp NOT in rules[] — LiveRuleStore.

        let rule_set = profile.rule_sets.as_ref().map(|rss| {
            rss.iter()
                .filter(|rs| rs.enabled)
                .map(|rs| {
                    if rs.kind == "local" {
                        RouteRuleSet::Local(LocalRouteRuleSet {
                            tag: rs.tag.clone(),
                            format: Some(RuleSetFormat::Binary),
                            path: rs.source.clone(),
                            rule_set_type: LocalRouteRuleSetType::Local,
                        })
                    } else {
                        let detour = rs
                            .download_policy
                            .as_deref()
                            .filter(|s| !s.is_empty())
                            .unwrap_or("direct");
                        // Modern API (1.14+): http_client, not download_detour.
                        // detour=direct via DetourDialer fails empty-direct check
                        // (DisableEmptyDirectCheck is json:"-" / not settable). Omit
                        // detour for direct → plain dialer. Proxy: { detour }.
                        let http_client = if detour == "direct" {
                            None
                        } else {
                            Some(HttpClientRef::Fields(serde_json::json!({
                                "detour": detour,
                            })))
                        };
                        RouteRuleSet::Remote(RemoteRouteRuleSet {
                            tag: rs.tag.clone(),
                            format: Some(RuleSetFormat::Binary),
                            url: rs.source.clone(),
                            rule_set_type: RemoteRouteRuleSetType::Remote,
                            download_detour: None,
                            update_interval: Some("24h".to_string()),
                            http_client,
                        })
                    }
                })
                .collect()
        });

        let live_rules = {
            let now_ms = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis() as i64)
                .unwrap_or(0);
            // Temp rules bake ONLY in rule mode. LiveRuleStore matches temp
            // BEFORE the clash-mode catch-alls, so baking temp while a
            // non-rule mode is active would leak it past Direct/Global
            // (runtime-proven: `match[temp]` fires in direct mode).
            let temp: Vec<_> = if *outbound_mode == OutboundMode::Rule {
                profile
                    .live_temp
                    .iter()
                    .filter(|r| r.enabled && !r.is_expired(now_ms))
                    .map(|r| {
                        let model = crate::core::compiler::profile_converter::ProfileConverter::core_rule_to_model_pub(
                            &r.to_rule(),
                        );
                        crate::core::models::zeytun_core::route::LiveRuleEntry {
                            id: r.id,
                            expires_at: r.expires_at,
                            rule: Self::temp_rule_json(&model),
                        }
                    })
                    .collect()
            } else {
                Vec::new()
            };
            let permanent: Vec<_> = profile
                .live_permanent
                .iter()
                .filter(|r| {
                    r.enabled && !matches!(r.kind, crate::core::dto::RuleType::Final)
                })
                .map(|r| {
                    let model = crate::core::compiler::profile_converter::ProfileConverter::core_rule_to_model_pub(r);
                    crate::core::models::zeytun_core::route::LiveRuleEntry {
                        id: r.id,
                        expires_at: 0,
                        rule: Self::temp_rule_json(&model),
                    }
                })
                .collect();
            if temp.is_empty() && permanent.is_empty() {
                None
            } else {
                Some(crate::core::models::zeytun_core::route::LiveRulesOptions { temp, permanent })
            }
        };

        Route {
            rules: (!rules.is_empty()).then_some(rules),
            rule_set,
            final_outbound: Some(profile.final_outbound.clone()),
            auto_detect_interface: Some(true),
            find_process: Some(true),
            connection_ask: profile.connection_ask.as_ref().and_then(|a| {
                // Always bake ask into config when enabled. In Direct/Global
                // the clash_mode catch-all matches everything BEFORE ask can
                // trigger, so ask is unreachable — safe to include. The gate
                // must NOT filter on outbound_mode because the app boots in
                // Direct (runStartupSequence) and PATCH-switches to Rule
                // without rebuilding config; if ask wasn't baked at boot, it
                // can never fire after the PATCH.
                if !a.enabled {
                    return None;
                }
                Some(
                    crate::core::models::zeytun_core::route::ConnectionAskOptions {
                        enabled: true,
                        timeout_ms: Some(a.timeout_ms.max(1000)),
                        group_by: Some(if a.group_by == "process_dest" {
                            "process_dest".to_string()
                        } else {
                            "process".to_string()
                        }),
                        on_timeout: Some("final".to_string()),
                    },
                )
            }),
            live_rules,
            ..Default::default()
        }
    }

    fn build_clash_mode_rules(&self, global_outbound: &str) -> Vec<ZeytunCoreRouteRule> {
        vec![
            // direct mode: bypass all policy/rule routing and send traffic directly.
            ZeytunCoreRouteRule::Default(DefaultRouteRule {
                clash_mode: Some(ClashMode::Direct),
                action: RuleAction::Route {
                    outbound: "direct".to_string(),
                    route_options: RouteOptionsAction::default(),
                },
                ..Self::empty_route_rule()
            }),
            // global mode: force all traffic to selected/global outbound.
            ZeytunCoreRouteRule::Default(DefaultRouteRule {
                clash_mode: Some(ClashMode::Global),
                action: RuleAction::Route {
                    outbound: global_outbound.to_string(),
                    route_options: RouteOptionsAction::default(),
                },
                ..Self::empty_route_rule()
            }),
            // rule mode: no forced outbound, continue with user-defined routing rules.
            ZeytunCoreRouteRule::Default(DefaultRouteRule {
                clash_mode: Some(ClashMode::Rule),
                // Sentinel: keep `rule` in mode-list without matching real traffic.
                domain: Some(vec!["zeytun-mode-rule-sentinel.invalid".to_string()]),
                action: RuleAction::Route {
                    outbound: "direct".to_string(),
                    route_options: RouteOptionsAction::default(),
                },
                ..Self::empty_route_rule()
            }),
        ]
    }

    fn empty_route_rule() -> DefaultRouteRule {
        DefaultRouteRule {
            inbound: None,
            ip_version: None,
            network: None,
            auth_user: None,
            protocol: None,
            client: None,
            domain: None,
            domain_suffix: None,
            domain_keyword: None,
            domain_regex: None,
            source_ip_cidr: None,
            source_ip_is_private: None,
            ip_cidr: None,
            ip_is_private: None,
            source_port: None,
            source_port_range: None,
            port: None,
            port_range: None,
            process_name: None,
            process_path: None,
            process_path_regex: None,
            package_name: None,
            package_name_regex: None,
            user: None,
            user_id: None,
            clash_mode: None,
            network_type: None,
            network_is_expensive: None,
            network_is_constrained: None,
            interface_address: None,
            network_interface_address: None,
            default_interface_address: None,
            wifi_ssid: None,
            wifi_bssid: None,
            preferred_by: None,
            source_mac_address: None,
            source_hostname: None,
            rule_set: None,
            rule_set_ip_cidr_match_source: None,
            invert: None,
            action: RuleAction::RouteOptions {
                route_options: RouteOptionsAction::default(),
            },
        }
    }

    fn build_route_rule(rule: &ModelRouteRule) -> ZeytunCoreRouteRule {
        let values = split_rule_values(&rule.value);

        let mut default = DefaultRouteRule {
            inbound: None,
            ip_version: None,
            network: None,
            auth_user: None,
            protocol: None,
            client: None,
            domain: None,
            domain_suffix: None,
            domain_keyword: None,
            domain_regex: None,
            source_ip_cidr: None,
            source_ip_is_private: None,
            ip_cidr: None,
            ip_is_private: None,
            source_port: None,
            source_port_range: None,
            port: None,
            port_range: None,
            process_name: None,
            process_path: None,
            process_path_regex: None,
            package_name: None,
            package_name_regex: None,
            user: None,
            user_id: None,
            clash_mode: None,
            network_type: None,
            network_is_expensive: None,
            network_is_constrained: None,
            interface_address: None,
            network_interface_address: None,
            default_interface_address: None,
            wifi_ssid: None,
            wifi_bssid: None,
            preferred_by: None,
            source_mac_address: None,
            source_hostname: None,
            rule_set: None,
            rule_set_ip_cidr_match_source: None,
            invert: None,
            action: convert_action(&rule.action.action),
        };

        match &rule.rule_type {
            RuleType::Domain => default.domain = Some(values),
            RuleType::DomainSuffix => default.domain_suffix = Some(values),
            RuleType::DomainKeyword => default.domain_keyword = Some(values),
            RuleType::DomainRegex => default.domain_regex = Some(values),
            RuleType::IpCidr => default.ip_cidr = Some(values),
            RuleType::InPort => {
                let (ports, ranges) = split_ports(values);
                if !ports.is_empty() {
                    default.source_port = Some(ports);
                }
                if !ranges.is_empty() {
                    default.source_port_range = Some(ranges);
                }
            }
            RuleType::DestPort => {
                let (ports, ranges) = split_ports(values);
                if !ports.is_empty() {
                    default.port = Some(ports);
                }
                if !ranges.is_empty() {
                    default.port_range = Some(ranges);
                }
            }
            RuleType::GeoIp => default.rule_set = Some(as_rule_set_tags(values, "geoip")),
            RuleType::GeoSite => default.rule_set = Some(as_rule_set_tags(values, "geosite")),
            RuleType::ProcessName => default.process_name = Some(values),
            RuleType::ProcessPath => default.process_path = Some(values),
            RuleType::ProcessPathRegex => default.process_path_regex = Some(values),
            RuleType::Protocol => default.protocol = Some(values),
        }

        ZeytunCoreRouteRule::Default(default)
    }

    /// Serialize a model rule as a sing-box default route rule JSON object
    /// (for Clash PUT /temp-rules `rule` field).
    pub fn temp_rule_json(rule: &ModelRouteRule) -> serde_json::Value {
        match Self::build_route_rule(rule) {
            ZeytunCoreRouteRule::Default(d) => {
                serde_json::to_value(d).unwrap_or(serde_json::Value::Null)
            }
            ZeytunCoreRouteRule::Logical(l) => {
                serde_json::to_value(l).unwrap_or(serde_json::Value::Null)
            }
        }
    }
}

fn split_rule_values(value: &str) -> Vec<String> {
    value
        .split([',', '\n'])
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

fn split_ports(values: Vec<String>) -> (Vec<u16>, Vec<String>) {
    let mut ports = Vec::new();
    let mut ranges = Vec::new();

    for value in values {
        if let Ok(port) = value.parse::<u16>() {
            ports.push(port);
        } else {
            ranges.push(value);
        }
    }

    (ports, ranges)
}

fn as_rule_set_tags(values: Vec<String>, prefix: &str) -> Vec<String> {
    values
        .into_iter()
        .map(|value| {
            if value.starts_with("geoip-") || value.starts_with("geosite-") {
                value
            } else {
                format!("{}-{}", prefix, value)
            }
        })
        .collect()
}

fn convert_action(action: &RuleActionType) -> RuleAction {
    match action {
        RuleActionType::Route(route) => RuleAction::Route {
            outbound: route.outbound.clone(),
            route_options: RouteOptionsAction::default(),
        },
        RuleActionType::Reject(reject) => RuleAction::Reject {
            method: reject.method.as_ref().map(convert_reject_method),
            no_drop: reject.no_drop,
        },
        RuleActionType::Bypass => RuleAction::Bypass {
            outbound: None,
            route_options: RouteOptionsAction::default(),
        },
        RuleActionType::HijackDns => RuleAction::HijackDns,
    }
}

fn convert_reject_method(method: &ModelRejectMethod) -> ZeytunCoreRejectMethod {
    match method {
        ModelRejectMethod::Default => ZeytunCoreRejectMethod::Default,
        ModelRejectMethod::Drop => ZeytunCoreRejectMethod::Drop,
        ModelRejectMethod::Reply => ZeytunCoreRejectMethod::Reply,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{
        dto::{DnsConfig, DnsHostEntry, DnsRule as ProfileDnsRule, DnsServer as ProfileDnsServer},
        models::{
            runtime::RuntimeProfile,
            zeytun_core::{dns::DnsRule, log::Log},
        },
    };

    fn profile_dns_rule(
        id: &str,
        kind: &str,
        value: &str,
        target: &str,
        enabled: bool,
    ) -> ProfileDnsRule {
        ProfileDnsRule {
            id: id.to_string(),
            kind: kind.to_string(),
            value: value.to_string(),
            target: target.to_string(),
            comment: None,
            enabled,
        }
    }

    fn runtime_profile_with_dns(rules: Vec<ProfileDnsRule>) -> RuntimeProfile {
        RuntimeProfile {
            name: "DNS conversion test".to_string(),
            log: Log::default(),
            tags: Vec::new(),
            inbound_mode: Default::default(),
            outbounds: Vec::new(),
            policies: Vec::new(),
            rules: Vec::new(),
            live_temp: Vec::new(),
            live_permanent: Vec::new(),
            rule_sets: None,
            final_outbound: "direct".to_string(),
            dns: Some(DnsConfig {
                mode: Some(crate::core::dto::DnsMode::Manual),
                servers: Some(vec![
                    ProfileDnsServer {
                        tag: "primary".to_string(),
                        address: "1.1.1.1".to_string(),
                        detour: None,
                        name: "Primary DNS".to_string(),
                    },
                    ProfileDnsServer {
                        tag: "secondary".to_string(),
                        address: "8.8.8.8".to_string(),
                        detour: None,
                        name: "Secondary DNS".to_string(),
                    },
                ]),
                final_server: Some("primary".to_string()),
                fake_ip: None,
                rules: Some(rules),
                hosts: None,
            }),
            connection_ask: None,
            ignored_proxy_servers: Vec::new(),
            warnings: Vec::new(),
        }
    }

    #[test]
    fn build_dns_converts_enabled_rules_in_order() {
        let profile = runtime_profile_with_dns(vec![
            profile_dns_rule("domain", "domain", "example.com", "primary", true),
            profile_dns_rule("disabled", "domain", "ignored.example", "primary", false),
            profile_dns_rule("suffix", "domain_suffix", ".example.org", "block", true),
            profile_dns_rule("ruleset", "ruleset", "enabled-ruleset", "secondary", true),
        ]);

        let rules = ZeytunCoreConfigBuilder::new(false, false)
            .build_dns(&profile)
            .expect("DNS config")
            .rules
            .expect("compiled DNS rules");
        assert!(rules.iter().all(|rule| matches!(rule, DnsRule::Default(_))));
        assert_eq!(
            serde_json::to_value(&rules[..3]).expect("serialize DNS rules"),
            serde_json::json!([
                {
                    "domain": ["example.com"],
                    "action": "route",
                    "server": "primary"
                },
                {
                    "domain_suffix": [".example.org"],
                    "action": "reject"
                },
                {
                    "rule_set": ["enabled-ruleset"],
                    "action": "route",
                    "server": "secondary"
                }
            ])
        );
    }

    #[test]
    fn build_json_groups_enabled_hosts_routes_them_first_and_persists_dns_cache() {
        let mut profile = runtime_profile_with_dns(vec![profile_dns_rule(
            "user-rule",
            "domain",
            "routed.example",
            "primary",
            true,
        )]);
        let dns = profile.dns.as_mut().expect("DNS config");
        dns.fake_ip = Some(true);
        dns.hosts = Some(vec![
            DnsHostEntry {
                id: "v4".to_string(),
                domain: "host.example".to_string(),
                address: "192.0.2.10".to_string(),
                enabled: true,
            },
            DnsHostEntry {
                id: "v6".to_string(),
                domain: "host.example".to_string(),
                address: "2001:db8::10".to_string(),
                enabled: true,
            },
            DnsHostEntry {
                id: "disabled".to_string(),
                domain: "disabled.example".to_string(),
                address: "192.0.2.20".to_string(),
                enabled: false,
            },
        ]);

        let config = ZeytunCoreConfigBuilder::new(false, false)
            .build(&profile, &OutboundMode::Rule)
            .expect("JSON config");
        let local_hosts = config["dns"]["servers"]
            .as_array()
            .expect("DNS servers")
            .iter()
            .find(|server| server["tag"] == DNS_HOSTS_SERVER_TAG)
            .expect("local hosts DNS server");
        assert_eq!(
            local_hosts,
            &serde_json::json!({
                "type": "hosts",
                "tag": "local-hosts",
                "predefined": {
                    "host.example": ["192.0.2.10", "2001:db8::10"]
                }
            })
        );

        let rules = config["dns"]["rules"].as_array().expect("DNS rules");
        assert_eq!(
            rules[0],
            serde_json::json!({
                "domain": ["host.example"],
                "action": "route",
                "server": "local-hosts"
            })
        );
        assert_eq!(rules[1]["domain"], serde_json::json!(["routed.example"]));
        assert_eq!(
            config["experimental"]["cache_file"]["store_dns"],
            serde_json::json!(true)
        );
        assert_eq!(
            config["experimental"]["cache_file"]["store_fakeip"],
            serde_json::json!(true)
        );
    }

    #[test]
    fn proxied_dns_hostname_uses_remote_resolution_instead_of_local_bootstrap() {
        for has_bootstrap in [false, true] {
            let mut profile = runtime_profile_with_dns(Vec::new());
            let dns = profile.dns.as_mut().unwrap();
            let mut servers = if has_bootstrap {
                dns.servers.take().unwrap()
            } else {
                Vec::new()
            };
            servers.push(ProfileDnsServer {
                tag: "google".into(),
                address: "https://dns.google/dns-query".into(),
                detour: Some("root-policy".into()),
                name: "Google DNS".into(),
            });
            dns.servers = Some(servers);
            dns.final_server = Some("google".into());
            let config = ZeytunCoreConfigBuilder::new(false, false)
                .build(&profile, &OutboundMode::Rule)
                .unwrap();
            let servers = config["dns"]["servers"].as_array().unwrap();
            let google = servers.iter().find(|s| s["tag"] == "google").unwrap();
            assert_eq!(google["server"], "dns.google");
            assert_eq!(google["detour"], "root-policy");
            assert!(google.get("domain_resolver").is_none());
            assert!(!servers
                .iter()
                .any(|s| s["tag"] == "builtin-fallback-resolver"));
        }
    }

    #[test]
    fn direct_dns_hostname_keeps_its_bootstrap_resolver() {
        for detour in [None, Some("direct")] {
            for has_bootstrap in [false, true] {
                let mut profile = runtime_profile_with_dns(Vec::new());
                let dns = profile.dns.as_mut().unwrap();
                let mut servers = if has_bootstrap {
                    dns.servers.take().unwrap()
                } else {
                    Vec::new()
                };
                servers.push(ProfileDnsServer {
                    tag: "google".into(),
                    address: "https://dns.google/dns-query".into(),
                    detour: detour.map(str::to_string),
                    name: "Google DNS".into(),
                });
                dns.servers = Some(servers);
                dns.final_server = Some("google".into());
                let config = ZeytunCoreConfigBuilder::new(false, false)
                    .build(&profile, &OutboundMode::Rule)
                    .unwrap();
                let google = config["dns"]["servers"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|s| s["tag"] == "google")
                    .unwrap();
                assert_eq!(
                    google["domain_resolver"],
                    if has_bootstrap {
                        "primary"
                    } else {
                        DNS_LOCAL_SERVER_TAG
                    }
                );
            }
        }
    }

    #[test]
    fn ipv6_dns_server_compiles_without_brackets_or_unrequested_bootstrap() {
        let mut profile = runtime_profile_with_dns(Vec::new());
        profile.dns.as_mut().unwrap().servers = Some(vec![ProfileDnsServer {
            tag: "ipv6".into(),
            name: "IPv6 DoH".into(),
            detour: None,
            address: "https://[2606:4700:4700::1111]:443/dns-query?key=value".into(),
        }]);
        profile.dns.as_mut().unwrap().final_server = Some("ipv6".into());
        let config = ZeytunCoreConfigBuilder::new(false, false)
            .build(&profile, &OutboundMode::Rule)
            .unwrap();
        let server = &config["dns"]["servers"][0];
        assert_eq!(server["server"], "2606:4700:4700::1111");
        assert_eq!(server["server_port"], 443);
        assert_eq!(server["path"], "/dns-query?key=value");
        assert!(server.get("domain_resolver").is_none());
    }

    #[test]
    fn fakeip_routes_address_queries_after_explicit_rules_and_local_domains() {
        let mut profile = runtime_profile_with_dns(vec![profile_dns_rule(
            "explicit",
            "domain",
            "updates.discord.com",
            "primary",
            true,
        )]);
        profile.dns.as_mut().unwrap().fake_ip = Some(true);
        let config = ZeytunCoreConfigBuilder::new(false, false)
            .build(&profile, &OutboundMode::Rule)
            .unwrap();
        let rules = config["dns"]["rules"].as_array().unwrap();
        assert_eq!(rules[0]["server"], "primary");
        assert_eq!(
            rules[1]["domain_suffix"],
            serde_json::json!(["local", "home.arpa"])
        );
        assert_eq!(rules[1]["server"], DNS_LOCAL_SERVER_TAG);
        assert_eq!(rules[2]["domain_regex"], serde_json::json!([r"^[^.]+\.?$"]));
        assert_eq!(rules[3]["clash_mode"], "direct");
        assert_eq!(rules[3]["server"], "primary");
        assert_eq!(rules[4]["query_type"], serde_json::json!(["A", "AAAA"]));
        assert_eq!(rules[4]["server"], "fakeip");
        assert_eq!(config["dns"]["final"], "primary");
    }

    /// A `hosts` server NXDOMAINs every domain outside its table, so it must never
    /// end up as the core's default resolver. With `dns.final` unset the core
    /// promotes the first registered server, which used to be the hosts server —
    /// every outbound's server domain then failed to resolve instantly.
    #[test]
    fn hosts_server_is_never_the_default_resolver() {
        let mut profile = runtime_profile_with_dns(Vec::new());
        let dns = profile.dns.as_mut().expect("profile DNS");
        dns.servers = None;
        dns.rules = None;
        dns.final_server = None;
        dns.hosts = Some(vec![DnsHostEntry {
            id: "v4".to_string(),
            domain: "host.example".to_string(),
            address: "192.0.2.10".to_string(),
            enabled: true,
        }]);

        let config = ZeytunCoreConfigBuilder::new(false, false)
            .build(&profile, &OutboundMode::Rule)
            .expect("JSON config");

        let servers = config["dns"]["servers"].as_array().expect("DNS servers");
        let final_tag = config["dns"]["final"]
            .as_str()
            .expect("dns.final must be set so the core does not pick the hosts server");
        let chosen = servers
            .iter()
            .find(|server| server["tag"] == final_tag)
            .expect("dns.final must name a declared server");
        assert_ne!(chosen["type"], "hosts");
        assert_ne!(chosen["type"], "fakeip");
        assert_eq!(chosen["type"], "local");
    }

    /// Explicit final DNS remains selected while Local/System is available for
    /// per-domain rules and direct hostname bootstrap.
    #[test]
    fn explicit_final_is_preserved_with_local_resolver_available() {
        let profile = runtime_profile_with_dns(Vec::new());
        assert_eq!(
            profile
                .dns
                .as_ref()
                .and_then(|dns| dns.final_server.as_deref()),
            Some("primary"),
            "fixture must declare an explicit final server"
        );
        let config = ZeytunCoreConfigBuilder::new(false, false)
            .build(&profile, &OutboundMode::Rule)
            .expect("JSON config");

        let servers = config["dns"]["servers"].as_array().expect("DNS servers");
        assert!(
            servers
                .iter()
                .any(|server| server["tag"] == DNS_LOCAL_SERVER_TAG),
            "local DNS must remain available without overriding the explicit final"
        );
        assert_eq!(config["dns"]["final"], serde_json::json!("primary"));
    }

    /// `dns.final` unset means the built-in local resolver, never "whatever the
    /// core picks first".
    #[test]
    fn unset_final_defaults_to_local_resolver() {
        let mut profile = runtime_profile_with_dns(Vec::new());
        profile.dns.as_mut().expect("profile DNS").final_server = None;
        let config = ZeytunCoreConfigBuilder::new(false, false)
            .build(&profile, &OutboundMode::Rule)
            .expect("JSON config");

        assert_eq!(
            config["dns"]["final"],
            serde_json::json!(DNS_LOCAL_SERVER_TAG)
        );
        let servers = config["dns"]["servers"].as_array().expect("DNS servers");
        let local = servers
            .iter()
            .find(|server| server["tag"] == DNS_LOCAL_SERVER_TAG)
            .expect("local resolver must be declared");
        assert_eq!(local["type"], "local");
    }

    /// A `final` tag naming a deleted server must not leave the core to choose.
    #[test]
    fn unknown_final_tag_is_rejected_without_silent_local_fallback() {
        let mut profile = runtime_profile_with_dns(Vec::new());
        profile.dns.as_mut().expect("profile DNS").final_server = Some("deleted-server".into());
        assert!(ZeytunCoreConfigBuilder::new(false, false)
            .build(&profile, &OutboundMode::Rule)
            .is_err());
    }
}
