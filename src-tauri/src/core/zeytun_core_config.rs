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
            dns: self.build_dns(profile),
            services,
            experimental: Some(experimental),
        };

        serde_json::to_value(config)
            .map_err(|e| format!("failed to serialize zeytun-core config: {e}"))
    }

    fn build_dns(&self, profile: &RuntimeProfile) -> Dns {
        use crate::core::models::zeytun_core::dns::{
            DnsRule, DnsRuleAction, DnsServer, DnsServerType,
        };

        let mut dns = Dns::default();

        if let Some(profile_dns) = &profile.dns {
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

            // Remote servers
            let mut domain_resolver_tag = None;
            if let Some(custom_servers) = &profile_dns.servers {
                domain_resolver_tag = custom_servers.iter().find_map(|s| {
                    let mut host = s.address.as_str();
                    for prefix in &["tls://", "https://", "tcp://", "quic://", "udp://"] {
                        if let Some(rest) = host.strip_prefix(prefix) {
                            host = rest;
                            break;
                        }
                    }
                    let host_no_path = host.split('/').next().unwrap_or("");
                    let host_no_port = host_no_path.split(':').next().unwrap_or("");
                    if host_no_port.parse::<std::net::IpAddr>().is_ok() {
                        Some(s.tag.clone())
                    } else {
                        None
                    }
                });
            }

            let fallback_resolver_tag = "builtin-fallback-resolver".to_string();
            let mut needs_fallback = false;

            if let Some(custom_servers) = &profile_dns.servers {
                for server in custom_servers {
                    let mut options = std::collections::HashMap::new();
                    let (server_type, server_host, path) =
                        if let Some(rest) = server.address.strip_prefix("tls://") {
                            (DnsServerType::Tls, rest.to_string(), None)
                        } else if let Some(rest) = server.address.strip_prefix("https://") {
                            // Extract host and path for https
                            let (host, _port, p) = split_host_port_path(rest);
                            (DnsServerType::Https, host, p)
                        } else if let Some(rest) = server.address.strip_prefix("tcp://") {
                            (DnsServerType::Tcp, rest.to_string(), None)
                        } else if let Some(rest) = server.address.strip_prefix("quic://") {
                            (DnsServerType::Quic, rest.to_string(), None)
                        } else if let Some(rest) = server.address.strip_prefix("udp://") {
                            (DnsServerType::Udp, rest.to_string(), None)
                        } else {
                            (DnsServerType::Udp, server.address.clone(), None)
                        };

                    // Strip port from server_host if present
                    let (host, port, _path) = split_host_port_path(&server_host);

                    options.insert(
                        "server".to_string(),
                        serde_json::Value::String(host.clone()),
                    );
                    if let Some(p_num) = port {
                        options.insert(
                            "server_port".to_string(),
                            serde_json::Value::Number(p_num.into()),
                        );
                    }
                    if let Some(p) = path {
                        options.insert("path".to_string(), serde_json::Value::String(p));
                    }

                    if let Some(detour) = &server.detour {
                        options.insert(
                            "detour".to_string(),
                            serde_json::Value::String(detour.clone()),
                        );
                    }

                    let is_ip = host.parse::<std::net::IpAddr>().is_ok();
                    if !is_ip {
                        if let Some(resolver_tag) = &domain_resolver_tag {
                            options.insert(
                                "domain_resolver".to_string(),
                                serde_json::Value::String(resolver_tag.clone()),
                            );
                        } else {
                            options.insert(
                                "domain_resolver".to_string(),
                                serde_json::Value::String(fallback_resolver_tag.clone()),
                            );
                            needs_fallback = true;
                        }
                    }

                    servers.push(DnsServer {
                        server_type,
                        tag: Some(server.tag.clone()),
                        options,
                    });
                }
            }

            if needs_fallback {
                let mut fallback_opts = std::collections::HashMap::new();
                fallback_opts.insert(
                    "server".to_string(),
                    serde_json::Value::String("223.5.5.5".to_string()),
                );
                fallback_opts.insert(
                    "detour".to_string(),
                    serde_json::Value::String("direct".to_string()),
                );
                servers.push(DnsServer {
                    server_type: DnsServerType::Udp,
                    tag: Some(fallback_resolver_tag),
                    options: fallback_opts,
                });
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
                    rules.push(DnsRule::Default(default));
                }
            }

            // `dns.final` is the resolver for every query no rule matched. It is
            // surfaced in the UI as the last, always-enabled DNS Rules row.
            //
            // A `hosts` server NXDOMAINs every domain outside its table
            // (`dns/transport/hosts/hosts.go:98`), and the core promotes the FIRST
            // registered server to default when `dns.final` is unset
            // (`dns/transport_manager.go:284`) — which silently made the hosts
            // server resolve everything. So `final` is always written explicitly,
            // and the built-in local resolver is the default.
            let is_real_resolver = |server: &DnsServer| {
                !matches!(
                    server.server_type,
                    DnsServerType::Hosts | DnsServerType::Fakeip
                )
            };
            let requested_final = profile_dns
                .final_server
                .as_deref()
                .map(str::trim)
                .filter(|tag| !tag.is_empty())
                .unwrap_or(DNS_LOCAL_SERVER_TAG);
            // An unknown or non-resolver tag falls back to local rather than
            // letting the core pick for us.
            let final_tag = servers
                .iter()
                .find(|server| {
                    server.tag.as_deref() == Some(requested_final) && is_real_resolver(server)
                })
                .and_then(|server| server.tag.clone())
                .unwrap_or_else(|| DNS_LOCAL_SERVER_TAG.to_string());
            if final_tag == DNS_LOCAL_SERVER_TAG {
                servers.push(DnsServer {
                    server_type: DnsServerType::Local,
                    tag: Some(DNS_LOCAL_SERVER_TAG.to_string()),
                    options: std::collections::HashMap::new(),
                });
            }

            if !servers.is_empty() {
                dns.servers = Some(servers);
            }

            if !rules.is_empty() {
                dns.rules = Some(rules);
            }
            dns.final_server = Some(final_tag);
        }

        dns
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
        let global_outbound = profile
            .policies
            .iter()
            .find(|policy| policy.source_proxy_policy_id == DEFAULT_SELECTOR_POLICY_TAG)
            .map(|policy| policy.tag.clone())
            .or_else(|| profile.policies.first().map(|policy| policy.tag.clone()))
            .or_else(|| {
                profile
                    .outbounds
                    .first()
                    .map(|outbound| outbound.tag.clone())
            })
            .unwrap_or_else(|| "direct".to_string());

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

/// Split an address of the form `host[:port][/path]` into its parts.
///
/// Splits at the *last* colon so IPv6 literals keep their own colons:
/// `[::1]:443` is host `[::1]`, port 443 — a plain `split_once(':')` there
/// yields host `[` and a port that fails to parse, silently dropping the
/// port and shipping the whole address as the host.
fn split_host_port_path(address: &str) -> (String, Option<u16>, Option<String>) {
    // Path first — it can legitimately contain ':' (https://doh/dns-query).
    let (authority, path) = match address.split_once('/') {
        Some((a, p)) => (a, Some(format!("/{p}"))),
        None => (address, None),
    };

    // A bracketed IPv6 literal is its own authority: [::1]:443 or [::1].
    if let Some(rest) = authority.strip_prefix('[') {
        if let Some((host, after)) = rest.split_once(']') {
            let port = after.strip_prefix(':').and_then(|p| p.parse::<u16>().ok());
            return (format!("[{host}]"), port, path);
        }
    }

    // Plain host: the port, if any, is after the final colon.
    match authority.rsplit_once(':') {
        Some((host, port_str))
            if !host.is_empty() && !port_str.is_empty() && !port_str.contains(':') =>
        {
            match port_str.parse::<u16>() {
                Ok(port) => (host.to_string(), Some(port), path),
                Err(_) => (authority.to_string(), None, path),
            }
        }
        _ => (authority.to_string(), None, path),
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
    fn split_host_port_path_handles_every_address_shape() {
        // IPv6 was the actual bug: split_once(':') gave host "[" and a port
        // that never parsed, so the whole address shipped as the host.
        let cases: &[(&str, &str, Option<u16>, Option<&str>)] = &[
            ("1.2.3.4", "1.2.3.4", None, None),
            ("1.2.3.4:443", "1.2.3.4", Some(443), None),
            ("dns.example.com", "dns.example.com", None, None),
            ("dns.example.com:853", "dns.example.com", Some(853), None),
            ("[::1]:443", "[::1]", Some(443), None),
            ("[::1]", "[::1]", None, None),
            ("[2001:db8::1]:853", "[2001:db8::1]", Some(853), None),
            // A path with no port.
            (
                "dns.example.com/dns-query",
                "dns.example.com",
                None,
                Some("/dns-query"),
            ),
            // Port and path together.
            (
                "dns.example.com:443/dns-query",
                "dns.example.com",
                Some(443),
                Some("/dns-query"),
            ),
            // IPv6 with a path.
            (
                "[::1]:443/dns-query",
                "[::1]",
                Some(443),
                Some("/dns-query"),
            ),
            // Empty input must not panic.
            ("", "", None, None),
            // A port that is not a number is left alone, not truncated.
            (
                "dns.example.com:notaport",
                "dns.example.com:notaport",
                None,
                None,
            ),
        ];

        for (input, host, port, path) in cases {
            let (got_host, got_port, got_path) = split_host_port_path(input);
            assert_eq!(
                got_host.as_str(),
                *host,
                "host for {input:?}: got {got_host:?}, want {host:?}"
            );
            assert_eq!(got_port, *port, "port for {input:?}");
            assert_eq!(
                got_path.as_deref(),
                *path,
                "path for {input:?}: got {got_path:?}, want {path:?}"
            );
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
            .rules
            .expect("compiled DNS rules");
        assert!(rules.iter().all(|rule| matches!(rule, DnsRule::Default(_))));
        assert_eq!(
            serde_json::to_value(rules).expect("serialize DNS rules"),
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

    /// An explicit `dns.final` is honoured verbatim and suppresses the injected
    /// local resolver. (With `final` unset the default is local by design — the
    /// user-facing rule is "Local (System)" unless the last DNS Rules row says
    /// otherwise.)
    #[test]
    fn existing_resolver_is_preferred_over_injected_local() {
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
            !servers
                .iter()
                .any(|server| server["tag"] == DNS_LOCAL_SERVER_TAG),
            "must not inject a local resolver when `dns.final` names a real one"
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
    fn unknown_final_tag_falls_back_to_local() {
        let mut profile = runtime_profile_with_dns(Vec::new());
        profile.dns.as_mut().expect("profile DNS").final_server =
            Some("deleted-server".to_string());
        let config = ZeytunCoreConfigBuilder::new(false, false)
            .build(&profile, &OutboundMode::Rule)
            .expect("JSON config");

        assert_eq!(
            config["dns"]["final"],
            serde_json::json!(DNS_LOCAL_SERVER_TAG)
        );
    }
}
