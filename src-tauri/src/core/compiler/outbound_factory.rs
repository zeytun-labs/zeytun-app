use crate::core::{
    compiler::error::CompileError,
    models::{
        policy::{self, ProxyPolicy},
        proxy::{Fingerprint, Protocol, ProxyServer, TransportSettings},
        zeytun_core::{
            outbound::{
                balancer::BalancerOutbound,
                http::HttpOutbound,
                hysteria2::{Hysteria2Obfs, Hysteria2ObfsType, Hysteria2Outbound},
                selector::SelectorOutbound,
                shadowsocks::{ShadowsocksMethod, ShadowsocksOutbound, ShadowsocksPlugin},
                socks::{SocksOutbound, SocksVersion},
                trojan::TrojanOutbound,
                tuic::{TuicCongestionControl, TuicOutbound, TuicUdpRelayMode},
                urltest::UrlTestOutbound,
                vless::{VlessFlow, VlessOutbound},
                vmess::{VmessOutbound, VmessSecurity},
                Outbound, OutboundType,
            },
            shared::{
                brutal::TcpBrutalOptions,
                dial::DialOptions,
                mux::MultiplexOptions,
                network::NetworkOptions,
                packet_encoding::PacketEncoding,
                quic::QuicOptions,
                tls::{
                    OutboundEchOptions, OutboundRealityOptions, OutboundTlsOptions,
                    OutboundUtlsFingerprint, OutboundUtlsOptions, TlsVersion,
                },
                transport::V2RayTransport,
                udp_over_tcp::{UdpOverTcp, UdpOverTcpOptions},
            },
        },
    },
};

pub struct OutboundFactory;

impl OutboundFactory {
    pub fn build_policy_outbound(
        &self,
        policy: &ProxyPolicy,
        tag: String,
    ) -> Result<Outbound, CompileError> {
        match &policy.policy_type {
            policy::ProxyPolicyType::Manual(_) => self.build_selector(policy, tag),
            policy::ProxyPolicyType::Auto(_) => self.build_urltest(policy, tag),
            policy::ProxyPolicyType::Balancer(_) => self.build_balancer(policy, tag),
        }
    }

    pub fn build_proxy_outbound(
        &self,
        server: &ProxyServer,
        tag: String,
    ) -> Result<Outbound, CompileError> {
        match &server.protocol {
            Protocol::Socks(_) => self.build_socks(server, tag),
            Protocol::Http(_) => self.build_http(server, tag),
            Protocol::Shadowsocks(_) => self.build_shadowsocks(server, tag),
            Protocol::Trojan(_) => self.build_trojan(server, tag),
            Protocol::Hysteria2(_) => self.build_hysteria2(server, tag),
            Protocol::Tuic(_) => self.build_tuic(server, tag),
            Protocol::Vless(_) => self.build_vless(server, tag),
            Protocol::Vmess(_) => self.build_vmess(server, tag),
            Protocol::Chain(_) => Err(CompileError::UnsupportedProtocol {
                proxy_server_id: server.tag.clone(),
            }),
        }
    }

    fn build_shadowsocks(
        &self,
        server: &ProxyServer,
        tag: String,
    ) -> Result<Outbound, CompileError> {
        let Protocol::Shadowsocks(shadowsocks) = &server.protocol else {
            panic!();
        };

        let method: ShadowsocksMethod = parse_enum_from_str(
            &server.tag,
            "shadowsocks.encryption",
            &shadowsocks.encryption,
        )?;

        let plugin: Option<ShadowsocksPlugin> = parse_enum_from_optional_str(
            &server.tag,
            "shadowsocks.plugin",
            shadowsocks.plugin.as_deref(),
        )?;

        Ok(Outbound {
            tag,
            outbound_type: OutboundType::Shadowsocks(ShadowsocksOutbound {
                server: server.address.clone(),
                server_port: server.port,
                dial: build_dial_options(server),
                method,
                password: shadowsocks.password.clone(),
                plugin,
                plugin_opts: shadowsocks.plugin_args.clone(),
                network: None,
                udp_over_tcp: shadowsocks
                    .udp_over_tcp
                    .then_some(UdpOverTcp::Enabled(UdpOverTcpOptions::default())),
                multiplex: build_multiplex(
                    shadowsocks.mux,
                    shadowsocks.tcp_brutal,
                    shadowsocks.brutal_dl_speed,
                    shadowsocks.brutal_up_speed,
                ),
            }),
        })
    }

    fn build_trojan(&self, server: &ProxyServer, tag: String) -> Result<Outbound, CompileError> {
        let Protocol::Trojan(trojan) = &server.protocol else {
            panic!();
        };

        Ok(Outbound {
            tag,
            outbound_type: OutboundType::Trojan(TrojanOutbound {
                server: server.address.clone(),
                server_port: server.port,
                dial: build_dial_options(server),
                password: trojan.password.clone(),
                network: parse_network_option(&trojan.network),
                tls: trojan
                    .tls
                    .as_ref()
                    .map(|tls| convert_tls_options(tls, &server.address, server, true)),
                multiplex: build_multiplex(
                    trojan.mux,
                    trojan.tcp_brutal,
                    trojan.brutal_dl_speed,
                    trojan.brutal_up_speed,
                ),
                transport: parse_v2ray_transport(&trojan.network, trojan.transport.as_ref()),
            }),
        })
    }

    fn build_vless(&self, server: &ProxyServer, tag: String) -> Result<Outbound, CompileError> {
        let Protocol::Vless(vless) = &server.protocol else {
            panic!();
        };

        let flow: Option<VlessFlow> =
            parse_enum_from_optional_str(&server.tag, "vless.flow", vless.flow.as_deref())?;

        let packet_encoding: Option<PacketEncoding> = parse_enum_from_optional_str(
            &server.tag,
            "vless.packet_encoding",
            vless.packet_encoding.as_deref(),
        )?;

        Ok(Outbound {
            tag,
            outbound_type: OutboundType::Vless(VlessOutbound {
                server: server.address.clone(),
                server_port: server.port,
                dial: build_dial_options(server),
                uuid: vless.uuid.clone(),
                flow,
                network: parse_network_option(&vless.network),
                tls: vless
                    .tls
                    .as_ref()
                    .map(|tls| convert_tls_options(tls, &server.address, server, true)),
                packet_encoding,
                multiplex: build_multiplex(
                    vless.mux,
                    vless.tcp_brutal,
                    vless.brutal_dl_speed,
                    vless.brutal_up_speed,
                ),
                transport: parse_v2ray_transport(&vless.network, vless.transport.as_ref()),
            }),
        })
    }

    fn build_vmess(&self, server: &ProxyServer, tag: String) -> Result<Outbound, CompileError> {
        let Protocol::Vmess(vmess) = &server.protocol else {
            panic!();
        };

        let security: Option<VmessSecurity> =
            parse_enum_from_optional_str(&server.tag, "vmess.security", Some(&vmess.security))?;

        let packet_encoding: Option<PacketEncoding> = parse_enum_from_optional_str(
            &server.tag,
            "vmess.packet_encoding",
            vmess.packet_encoding.as_deref(),
        )?;

        Ok(Outbound {
            tag,
            outbound_type: OutboundType::Vmess(VmessOutbound {
                server: server.address.clone(),
                server_port: server.port,
                dial: build_dial_options(server),
                uuid: vmess.uuid.clone(),
                security,
                alter_id: Some(vmess.alter_id),
                global_padding: None,
                authenticated_length: None,
                network: parse_network_option(&vmess.network),
                tls: vmess
                    .tls
                    .as_ref()
                    .map(|tls| convert_tls_options(tls, &server.address, server, true)),
                packet_encoding,
                transport: parse_v2ray_transport(&vmess.network, vmess.transport.as_ref()),
                multiplex: build_multiplex(
                    vmess.mux,
                    vmess.tcp_brutal,
                    vmess.brutal_dl_speed,
                    vmess.brutal_up_speed,
                ),
            }),
        })
    }

    fn build_hysteria2(&self, server: &ProxyServer, tag: String) -> Result<Outbound, CompileError> {
        let Protocol::Hysteria2(hysteria2) = &server.protocol else {
            panic!();
        };

        let server_ports = hysteria2.server_ports.as_ref().map(|ports| {
            ports
                .split(',')
                .map(str::trim)
                .filter(|item| !item.is_empty())
                .map(ToOwned::to_owned)
                .collect::<Vec<_>>()
        });

        Ok(Outbound {
            tag,
            outbound_type: OutboundType::Hysteria2(Hysteria2Outbound {
                server: server.address.clone(),
                server_port: server.port,
                dial: build_dial_options(server),
                server_ports,
                hop_interval: hysteria2
                    .hop_interval
                    .as_deref()
                    .filter(|s| !s.is_empty())
                    .map(str::to_string),
                hop_interval_max: None,
                up_mbps: Some(hysteria2.up_mbps),
                down_mbps: Some(hysteria2.down_mbps),
                obfs: hysteria2
                    .obf_password
                    .as_ref()
                    .map(|password| Hysteria2Obfs {
                        obfs_type: Hysteria2ObfsType::Salamander,
                        password: password.clone(),
                    }),
                password: hysteria2.password.clone(),
                network: None,
                tls: convert_tls_options(&hysteria2.tls, &server.address, server, false),
                brutal_debug: None,
                bbr_profile: None,
                realm: None,
                quic: QuicOptions::default(),
            }),
        })
    }

    fn build_tuic(&self, server: &ProxyServer, tag: String) -> Result<Outbound, CompileError> {
        let Protocol::Tuic(tuic) = &server.protocol else {
            panic!();
        };

        let congestion_control: Option<TuicCongestionControl> = parse_enum_from_optional_str(
            &server.tag,
            "tuic.congestion_control",
            Some(&tuic.congestion_control),
        )?;

        let udp_relay_mode: Option<TuicUdpRelayMode> = parse_enum_from_optional_str(
            &server.tag,
            "tuic.udp_relay_mode",
            Some(&tuic.udp_relay_mode),
        )?;

        Ok(Outbound {
            tag,
            outbound_type: OutboundType::Tuic(TuicOutbound {
                server: server.address.clone(),
                server_port: server.port,
                dial: build_dial_options(server),
                uuid: tuic.uuid.clone(),
                password: Some(tuic.password.clone()),
                congestion_control,
                udp_relay_mode,
                udp_over_stream: Some(tuic.udp_over_stream),
                zero_rtt_handshake: Some(tuic.zero_rtt_handshake),
                heartbeat: tuic.heartbeat.clone(),
                network: None,
                tls: convert_tls_options(&tuic.tls, &server.address, server, false),
                quic: QuicOptions::default(),
            }),
        })
    }

    fn build_socks(&self, server: &ProxyServer, tag: String) -> Result<Outbound, CompileError> {
        let Protocol::Socks(socks) = &server.protocol else {
            panic!();
        };

        let version = match socks.version {
            4 => Some(SocksVersion::V4),
            5 => Some(SocksVersion::V5),
            _ => {
                return Err(CompileError::InvalidProxyServerField {
                    proxy_server_id: server.tag.clone(),
                    field: "socks.version",
                    value: socks.version.to_string(),
                });
            }
        };

        Ok(Outbound {
            tag,
            outbound_type: OutboundType::Socks(SocksOutbound {
                server: server.address.clone(),
                server_port: server.port,
                dial: build_dial_options(server),
                version,
                username: (!socks.username.is_empty()).then_some(socks.username.clone()),
                password: (!socks.password.is_empty()).then_some(socks.password.clone()),
                network: None,
                udp_over_tcp: None,
            }),
        })
    }

    fn build_http(&self, server: &ProxyServer, tag: String) -> Result<Outbound, CompileError> {
        let Protocol::Http(http) = &server.protocol else {
            panic!();
        };

        Ok(Outbound {
            tag,
            outbound_type: OutboundType::Http(HttpOutbound {
                server: server.address.clone(),
                server_port: server.port,
                dial: build_dial_options(server),
                path: None,
                username: (!http.username.is_empty()).then_some(http.username.clone()),
                password: (!http.password.is_empty()).then_some(http.password.clone()),
                headers: std::collections::HashMap::new(),
                tls: http
                    .tls
                    .as_ref()
                    .map(|tls| convert_tls_options(tls, &server.address, server, true)),
            }),
        })
    }

    fn build_selector(&self, policy: &ProxyPolicy, tag: String) -> Result<Outbound, CompileError> {
        let policy::ProxyPolicyType::Manual(manual) = &policy.policy_type else {
            panic!();
        };

        if policy.outbounds.is_empty() {
            return Err(CompileError::NoUsableOutbound);
        }

        if let Some(default) = manual.default.as_ref() {
            if !policy.outbounds.iter().any(|outbound| outbound == default) {
                return Err(CompileError::InvalidProxyServerField {
                    proxy_server_id: policy.tag.clone(),
                    field: "policy.manual.default",
                    value: default.clone(),
                });
            }
        }

        Ok(Outbound {
            tag,
            outbound_type: OutboundType::Selector(SelectorOutbound {
                outbounds: policy.outbounds.clone(),
                default: manual.default.clone(),
                interrupt_exist_connections: manual.interrupt_exist_connections,
            }),
        })
    }

    fn build_urltest(&self, policy: &ProxyPolicy, tag: String) -> Result<Outbound, CompileError> {
        let policy::ProxyPolicyType::Auto(auto) = &policy.policy_type else {
            panic!();
        };

        if policy.outbounds.is_empty() {
            return Err(CompileError::NoUsableOutbound);
        }

        let tolerance =
            match auto.tolerance {
                Some(value) => Some(u16::try_from(value).map_err(|_| {
                    CompileError::InvalidProxyServerField {
                        proxy_server_id: policy.tag.clone(),
                        field: "policy.auto.tolerance",
                        value: value.to_string(),
                    }
                })?),
                None => None,
            };

        Ok(Outbound {
            tag,
            outbound_type: OutboundType::Urltest(UrlTestOutbound {
                outbounds: policy.outbounds.clone(),
                url: auto.url.clone(),
                interval: auto.interval.clone(),
                tolerance,
                idle_timeout: auto.idle_timeout.clone(),
                interrupt_exist_connections: auto.interrupt_exist_connections,
            }),
        })
    }

    fn build_balancer(&self, policy: &ProxyPolicy, tag: String) -> Result<Outbound, CompileError> {
        let policy::ProxyPolicyType::Balancer(bal) = &policy.policy_type else {
            panic!();
        };

        if policy.outbounds.is_empty() {
            return Err(CompileError::NoUsableOutbound);
        }

        Ok(Outbound {
            tag,
            outbound_type: OutboundType::Balancer(BalancerOutbound {
                outbounds: policy.outbounds.clone(),
                strategy: Some(bal.strategy.clone()),
                tolerance: bal.tolerance,
                delay_acceptable_ratio: bal.delay_acceptable_ratio,
                ttl: bal.ttl.clone(),
                max_retry: bal.max_retry,
                weights: bal.weights.clone(),
                interrupt_exist_connections: bal.interrupt_exist_connections,
            }),
        })
    }
}

fn parse_enum_from_str<T>(
    proxy_server_id: &str,
    field: &'static str,
    value: &str,
) -> Result<T, CompileError>
where
    T: serde::de::DeserializeOwned,
{
    use serde::de::IntoDeserializer;
    T::deserialize(value.into_deserializer()).map_err(|_: serde::de::value::Error| {
        CompileError::InvalidProxyServerField {
            proxy_server_id: proxy_server_id.to_string(),
            field,
            value: value.to_string(),
        }
    })
}

fn parse_enum_from_optional_str<T>(
    proxy_server_id: &str,
    field: &'static str,
    value: Option<&str>,
) -> Result<Option<T>, CompileError>
where
    T: serde::de::DeserializeOwned,
{
    use serde::de::IntoDeserializer;

    let Some(value) = value else {
        return Ok(None);
    };

    let value = value.trim();

    if value.is_empty() || value.eq_ignore_ascii_case("none") {
        return Ok(None);
    }

    T::deserialize(value.into_deserializer())
        .map(Some)
        .map_err(
            |_: serde::de::value::Error| CompileError::InvalidProxyServerField {
                proxy_server_id: proxy_server_id.to_string(),
                field,
                value: value.to_string(),
            },
        )
}

fn parse_network_option(network: &str) -> Option<NetworkOptions> {
    parse_enum_from_optional_str::<NetworkOptions>("", "", Some(network))
        .ok()
        .flatten()
}

fn parse_v2ray_transport(
    network: &str,
    settings: Option<&TransportSettings>,
) -> Option<V2RayTransport> {
    // Phase2: single emit path in zeytun-link; app only deserializes into typed core models.
    let value = zeytun_config::transport_to_json(network, settings)?;
    serde_json::from_value(value).ok()
}

fn build_multiplex(
    mux: Option<bool>,
    tcp_brutal: Option<bool>,
    brutal_dl_speed: Option<u32>,
    brutal_up_speed: Option<u32>,
) -> Option<MultiplexOptions> {
    let enabled = mux.unwrap_or(false);
    if !enabled {
        return None;
    }

    let brutal = tcp_brutal.unwrap_or(false).then_some(TcpBrutalOptions {
        enabled: Some(true),
        up_mbps: brutal_up_speed,
        down_mbps: brutal_dl_speed,
    });

    Some(MultiplexOptions {
        enabled: true,
        protocol: None,
        max_connections: None,
        min_streams: None,
        max_streams: None,
        padding: None,
        brutal,
    })
}

fn build_dial_options(server: &ProxyServer) -> DialOptions {
    let advanced = server.advanced.unwrap_or(false);

    let mut opts = DialOptions {
        detour: server.detour.clone(),
        ..DialOptions::default()
    };

    if advanced {
        opts.reuse_addr = server.reuse_address;
        opts.tcp_fast_open = server.tcp_fast_open;
        opts.udp_fragment = server.udp_fragment;
        opts.tcp_multi_path = server.tcp_multi_path;
        opts.connect_timeout = server.connect_timeout.map(|ms| format!("{}ms", ms));
    }

    opts
}

fn parse_tls_version(version: &str) -> Option<TlsVersion> {
    match version.trim() {
        "1.0" => Some(TlsVersion::V1_0),
        "1.1" => Some(TlsVersion::V1_1),
        "1.2" => Some(TlsVersion::V1_2),
        "1.3" => Some(TlsVersion::V1_3),
        _ => None,
    }
}

fn convert_tls_options(
    tls: &crate::core::models::proxy::Tls,
    fallback_server_name: &str,
    server: &ProxyServer,
    support_utls_and_reality: bool,
) -> OutboundTlsOptions {
    let advanced = server.advanced.unwrap_or(false);

    let disable_sni = if advanced {
        server.tls_disable_sni
    } else {
        None
    };

    let min_version = if advanced {
        server.tls_min_version.as_deref().and_then(|v| {
            if v.is_empty() {
                None
            } else {
                parse_tls_version(v)
            }
        })
    } else {
        None
    };

    let max_version = if advanced {
        server.tls_max_version.as_deref().and_then(|v| {
            if v.is_empty() {
                None
            } else {
                parse_tls_version(v)
            }
        })
    } else {
        None
    };

    let ech = if advanced && server.tls_enable_ech.unwrap_or(false) {
        Some(OutboundEchOptions {
            enabled: Some(true),
            config: server
                .tls_ech_config
                .as_ref()
                .filter(|c| !c.is_empty())
                .map(|c| vec![c.clone()]),
            config_path: None,
            query_server_name: None,
        })
    } else {
        None
    };

    let certificate = {
        let mut certs: Vec<String> = Vec::new();
        if let Some(cert) = tls.certificate.as_ref() {
            certs.push(cert.clone());
        }
        if advanced {
            if let Some(sha) = server
                .tls_certificate_sha256
                .as_ref()
                .filter(|s| !s.is_empty())
            {
                certs.push(sha.clone());
            }
        }
        if certs.is_empty() {
            None
        } else {
            Some(certs)
        }
    };

    let client_certificate = if advanced {
        server
            .tls_client_cert
            .as_ref()
            .filter(|c| !c.is_empty())
            .map(|c| vec![c.clone()])
    } else {
        None
    };

    let client_key = if advanced {
        server
            .tls_client_key
            .as_ref()
            .filter(|k| !k.is_empty())
            .map(|k| vec![k.clone()])
    } else {
        None
    };

    OutboundTlsOptions {
        enabled: true,
        server_name: Some(
            tls.sni
                .clone()
                .unwrap_or_else(|| fallback_server_name.to_owned()),
        ),
        insecure: Some(tls.allow_insecure),
        alpn: tls.alpn.clone(),
        min_version,
        max_version,
        cipher_suites: None,
        certificate,
        certificate_path: None,
        disable_sni,
        ech,
        utls: if support_utls_and_reality {
            tls.fingerprint
                .as_ref()
                .map(|fingerprint| OutboundUtlsOptions {
                    enabled: Some(true),
                    fingerprint: Some(map_utls_fingerprint(fingerprint)),
                })
        } else {
            None
        },
        reality: if support_utls_and_reality {
            tls.reality_pbk
                .as_ref()
                .map(|public_key| OutboundRealityOptions {
                    enabled: true,
                    public_key: public_key.clone(),
                    short_id: tls.reality_sid.clone(),
                })
        } else {
            None
        },
        fragment: Some(tls.fragment),
        fragment_fallback_delay: tls.fallback_delay.clone(),
        record_fragment: Some(tls.record_fragment),
        handshake_timeout: None,
        client_certificate,
        client_certificate_path: None,
        client_key,
        client_key_path: None,
        engine: None,
        spoof: None,
        spoof_method: None,
    }
}

fn map_utls_fingerprint(fingerprint: &Fingerprint) -> OutboundUtlsFingerprint {
    match fingerprint {
        Fingerprint::Chrome => OutboundUtlsFingerprint::Chrome,
        Fingerprint::Firefox => OutboundUtlsFingerprint::Firefox,
        Fingerprint::Edge => OutboundUtlsFingerprint::Edge,
        Fingerprint::Qq => OutboundUtlsFingerprint::Qq,
        Fingerprint::Ios => OutboundUtlsFingerprint::Ios,
        Fingerprint::Android => OutboundUtlsFingerprint::Android,
        Fingerprint::Random => OutboundUtlsFingerprint::Random,
        Fingerprint::Randomized => OutboundUtlsFingerprint::Randomized,
        Fingerprint::ThreeSixty => OutboundUtlsFingerprint::ThreeSixty,
    }
}
