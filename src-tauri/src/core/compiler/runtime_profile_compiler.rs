use crate::core::models::zeytun_core::outbound::{Outbound, OutboundType};
use std::collections::{HashMap, HashSet, VecDeque};

use crate::core::{
    compiler::{
        error::CompileError, outbound_factory::OutboundFactory, support_resolver::SupportResolver,
        tag_allocator::TagAllocator, warning::CompileWarning,
    },
    models::{
        policy::{ProxyPolicy, ProxyPolicyType},
        profile::Profile,
        proxy::{Protocol, ProxyServer},
        rule::{RouteRule, RuleActionType},
        runtime::{IgnoredProxyServer, RuntimeOutbound, RuntimePolicy, RuntimeProfile},
    },
};

pub struct RuntimeProfileCompiler {
    support_resolver: SupportResolver,
    outbound_factory: OutboundFactory,
}

impl Default for RuntimeProfileCompiler {
    fn default() -> Self {
        Self::new()
    }
}

impl RuntimeProfileCompiler {
    /// All hot overlays use the same validated/resolved graph as cold config.
    pub fn live_rules_payload(
        profile: &crate::core::dto::ZeytunCore,
        include_session: bool,
    ) -> Result<serde_json::Value, String> {
        let mut snapshot = profile.clone();
        if include_session {
            snapshot.temp_rules.extend(snapshot.session_rules.clone());
        }
        let runtime = Self::new()
            .compile(
                &crate::core::compiler::profile_converter::ProfileConverter::convert(&snapshot),
            )
            .map_err(|e| e.to_string())?;
        let config = crate::core::zeytun_core_config::ZeytunCoreConfigBuilder::new(false, false)
            .build(&runtime, &snapshot.outbound_mode)?;
        Ok(config["route"]
            .get("live_rules")
            .cloned()
            .unwrap_or_else(|| serde_json::json!({"temp":[],"permanent":[]})))
    }

    pub fn new() -> Self {
        Self {
            support_resolver: SupportResolver,
            outbound_factory: OutboundFactory,
        }
    }

    pub fn compile(&self, profile: &Profile) -> Result<RuntimeProfile, CompileError> {
        if let Some(error) = &profile.validation_error {
            return Err(CompileError::InvalidProfile(error.clone()));
        }
        let mut tag_allocator = TagAllocator::new();

        let mut warnings = Vec::new();
        let mut ignored_proxy_servers = Vec::new();
        let mut outbounds = Vec::new();

        for policy in &profile.proxy_policies {
            tag_allocator.allocate_policy_tag(policy);
        }

        let mut regular_servers = Vec::new();
        let mut chain_servers = Vec::new();

        for server in &profile.proxy_servers {
            if let crate::core::models::proxy::Protocol::Chain(_) = &server.protocol {
                chain_servers.push(server.clone());
            } else {
                regular_servers.push(server.clone());
            }
        }

        // Reserve authored identities before emitting any references or chain
        // hops. Detours may point forward, and trimmed tags may collide.
        for server in regular_servers.iter().chain(&chain_servers) {
            tag_allocator.allocate_proxy_server_tag(server);
        }
        for id in &profile.blocked_proxy_tags {
            tag_allocator.allocate_proxy_tag(id, id);
        }

        for server in &regular_servers {
            let (mut normalized_server, normalize_warning) =
                self.support_resolver.normalize(server);

            if let Some(warning) = normalize_warning {
                warnings.push(CompileWarning::ProxyServerNormalized {
                    proxy_server_tag: server.tag.clone(),
                    name: server.name.clone(),
                    reason: warning,
                });
            }

            let support = self.support_resolver.resolve(&normalized_server);

            if !support.supported {
                let reason = support
                    .reason
                    .unwrap_or_else(|| "Unsupported proxy.".to_string());

                ignored_proxy_servers.push(IgnoredProxyServer {
                    tag: server.tag.clone(),
                    name: server.name.clone(),
                    reason: reason.clone(),
                });

                warnings.push(CompileWarning::ProxyServerIgnored {
                    proxy_server_tag: server.tag.clone(),
                    name: server.name.clone(),
                    reason,
                });

                continue;
            }

            let tag = tag_allocator.allocate_proxy_server_tag(&normalized_server);
            self.resolve_proxy_references(&mut normalized_server, &tag_allocator);

            let outbound = match self
                .outbound_factory
                .build_proxy_outbound(&normalized_server, tag.clone())
            {
                Ok(outbound) => outbound,
                Err(_) => {
                    warnings.push(CompileWarning::ProxyServerIgnored {
                        proxy_server_tag: server.tag.clone(),
                        name: server.name.clone(),
                        reason: "Invalid proxy configuration; traffic blocked.".into(),
                    });
                    Outbound {
                        tag: tag.clone(),
                        outbound_type: OutboundType::Block,
                    }
                }
            };

            outbounds.push(RuntimeOutbound {
                tag,
                source_proxy_server_id: server.tag.clone(),
                outbound,
            });
        }

        for chain_server in &chain_servers {
            let crate::core::models::proxy::Protocol::Chain(chain_proto) = &chain_server.protocol
            else {
                unreachable!()
            };

            if chain_proto.proxies.is_empty()
                || chain_proto.proxies.iter().any(|id| {
                    !outbounds.iter().any(|o| {
                        o.source_proxy_server_id == *id
                            && !matches!(o.outbound.outbound_type, OutboundType::Block)
                    })
                })
            {
                warnings.push(CompileWarning::ProxyServerIgnored {
                    proxy_server_tag: chain_server.tag.clone(),
                    name: chain_server.name.clone(),
                    reason: "Chain contains no proxies.".to_string(),
                });
                let tag = tag_allocator.allocate_proxy_server_tag(chain_server);
                outbounds.push(RuntimeOutbound {
                    tag: tag.clone(),
                    source_proxy_server_id: chain_server.tag.clone(),
                    outbound: Outbound {
                        tag,
                        outbound_type: OutboundType::Block,
                    },
                });
                continue;
            }

            let mut prev_allocated_tag = None;
            let mut chain_failed = false;

            for (i, node_tag) in chain_proto.proxies.iter().enumerate() {
                let Some(original_server) = regular_servers.iter().find(|s| s.tag == *node_tag)
                else {
                    warnings.push(CompileWarning::ProxyServerIgnored {
                        proxy_server_tag: chain_server.tag.clone(),
                        name: chain_server.name.clone(),
                        reason: format!(
                            "Chain node '{}' not found or is not a standard proxy.",
                            node_tag
                        ),
                    });
                    chain_failed = true;
                    break;
                };

                let (mut cloned_server, _) = self.support_resolver.normalize(original_server);
                self.resolve_proxy_references(&mut cloned_server, &tag_allocator);

                if i == 0 && chain_proto.proxies.len() > 1 {
                    prev_allocated_tag =
                        Some(tag_allocator.allocate_proxy_server_tag(original_server));
                    continue;
                }

                let is_last = i == chain_proto.proxies.len() - 1;

                if is_last {
                    cloned_server.tag = chain_server.tag.clone();
                    cloned_server.name = chain_server.name.clone();
                } else {
                    cloned_server.tag = format!("{}-{}", chain_server.tag, cloned_server.tag);
                    cloned_server.name = format!("{} - {}", chain_server.name, cloned_server.name);
                }

                cloned_server.detour = prev_allocated_tag.clone();

                let allocated_tag = if is_last {
                    tag_allocator.allocate_proxy_server_tag(&cloned_server)
                } else {
                    tag_allocator.allocate_chain_hop_tag(&chain_server.tag, node_tag)
                };

                match self
                    .outbound_factory
                    .build_proxy_outbound(&cloned_server, allocated_tag.clone())
                {
                    Ok(outbound) => {
                        outbounds.push(RuntimeOutbound {
                            tag: allocated_tag.clone(),
                            source_proxy_server_id: if is_last {
                                chain_server.tag.clone()
                            } else {
                                format!("{}-{}", chain_server.tag, node_tag)
                            },
                            outbound,
                        });
                        prev_allocated_tag = Some(allocated_tag);
                    }
                    Err(e) => {
                        warnings.push(CompileWarning::ProxyServerIgnored {
                            proxy_server_tag: chain_server.tag.clone(),
                            name: chain_server.name.clone(),
                            reason: format!("Failed to build chain node '{}': {:?}", node_tag, e),
                        });
                        chain_failed = true;
                        break;
                    }
                }
            }

            if chain_failed {
                ignored_proxy_servers.push(IgnoredProxyServer {
                    tag: chain_server.tag.clone(),
                    name: chain_server.name.clone(),
                    reason: "Failed to compile proxy chain".to_string(),
                });
                let tag = tag_allocator.allocate_proxy_server_tag(chain_server);
                outbounds.retain(|o| o.tag != tag);
                outbounds.push(RuntimeOutbound {
                    tag: tag.clone(),
                    source_proxy_server_id: chain_server.tag.clone(),
                    outbound: Outbound {
                        tag,
                        outbound_type: OutboundType::Block,
                    },
                });
            }
        }

        let mut valid_targets = Self::collect_builtin_tags();
        for outbound in &outbounds {
            if !matches!(outbound.outbound.outbound_type, OutboundType::Block) {
                valid_targets.insert(outbound.tag.clone());
            }
        }

        let policies = self.compile_policy(
            &profile.proxy_policies,
            &mut tag_allocator,
            &mut valid_targets,
            &mut warnings,
        )?;

        for id in &profile.blocked_proxy_tags {
            let tag = tag_allocator.allocate_proxy_tag(id, id);
            warnings.push(CompileWarning::ProxyServerIgnored {proxy_server_tag:id.clone(),name:id.clone(),reason:"Node is unavailable, disabled, invalid, or depends on an unusable node; traffic blocked.".into()});
            outbounds.push(RuntimeOutbound {
                tag: tag.clone(),
                source_proxy_server_id: id.clone(),
                outbound: Outbound {
                    tag,
                    outbound_type: OutboundType::Block,
                },
            });
        }
        valid_targets.extend(outbounds.iter().map(|o| o.tag.clone()));
        valid_targets.extend(policies.iter().map(|p| p.tag.clone()));

        let rules = self.compile_rules(
            &profile.rules,
            &tag_allocator,
            &valid_targets,
            &mut warnings,
        );
        let _ = self.compile_rules(
            &profile.temp_rules,
            &tag_allocator,
            &valid_targets,
            &mut warnings,
        );
        let final_outbound = self.compile_final_outbound(
            &profile.final_outbound,
            &tag_allocator,
            &valid_targets,
            &mut warnings,
        );

        let blocked_targets = outbounds
            .iter()
            .filter(|o| matches!(o.outbound.outbound_type, OutboundType::Block))
            .map(|o| o.tag.clone())
            .chain(
                policies
                    .iter()
                    .filter(|p| matches!(p.policy.outbound_type, OutboundType::Block))
                    .map(|p| p.tag.clone()),
            )
            .collect::<HashSet<_>>();
        let resolve = |target: &str| {
            self.resolve_target(target, &tag_allocator)
                .filter(|tag| valid_targets.contains(tag))
                // Hot live updates may reach a core still holding the previous
                // usable outbound at this tag. Use the built-in Block target,
                // not a placeholder that becomes Block only after restart.
                .map(|tag| {
                    if blocked_targets.contains(&tag) {
                        "block".into()
                    } else {
                        tag
                    }
                })
                .unwrap_or_else(|| "block".into())
        };
        let mut live_temp = profile.live_temp.clone();
        let mut live_permanent = profile.live_permanent.clone();
        for rule in &mut live_temp {
            if !["reject", "hijack-dns", "bypass"].contains(&rule.outbound.as_str()) {
                rule.outbound = resolve(&rule.outbound);
            }
        }
        for rule in &mut live_permanent {
            if !["reject", "hijack-dns", "bypass"].contains(&rule.outbound.as_str()) {
                rule.outbound = resolve(&rule.outbound);
            }
        }
        let mut dns = profile.dns.clone();
        for server in dns.iter_mut().flat_map(|d| d.servers.iter_mut().flatten()) {
            if let Some(target) = &mut server.detour {
                if target != crate::core::zeytun_core_config::DNS_FOLLOW_MODE {
                    *target = resolve(target);
                }
            }
        }
        let mut rule_sets = profile.rule_sets.clone();
        for rs in rule_sets.iter_mut().flatten() {
            rs.action = resolve(&rs.action);
            if let Some(target) = &mut rs.download_policy {
                *target = resolve(target);
            }
        }

        let mut tags: Vec<String> = valid_targets.iter().cloned().collect();
        tags.sort_unstable();

        Ok(RuntimeProfile {
            name: profile.name.clone(),
            log: crate::core::models::zeytun_core::log::Log {
                level: profile
                    .inbound_mode
                    .log_level
                    .as_deref()
                    .and_then(|l| {
                        serde_json::from_value(serde_json::Value::String(l.to_string())).ok()
                    })
                    .unwrap_or(crate::core::models::zeytun_core::log::LogLevel::Info),
                ..Default::default()
            },
            tags,
            inbound_mode: profile.inbound_mode.clone(),
            outbounds,
            policies,
            rules,
            live_temp,
            live_permanent,
            rule_sets,
            final_outbound,
            dns,
            connection_ask: profile.connection_ask.clone(),
            ignored_proxy_servers,
            warnings,
        })
    }

    fn compile_policy(
        &self,
        policies: &[ProxyPolicy],
        tag_allocator: &mut TagAllocator,
        valid_outbounds: &mut HashSet<String>,
        warnings: &mut Vec<CompileWarning>,
    ) -> Result<Vec<RuntimePolicy>, CompileError> {
        let mut runtime_groups = Vec::new();

        // Topological Sort (Kahn's algorithm)
        let mut in_degree = HashMap::new();
        let mut graph = HashMap::new();
        let mut policy_map = HashMap::new();

        for policy in policies {
            in_degree.insert(policy.tag.clone(), 0);
            graph.insert(policy.tag.clone(), Vec::new());
            policy_map.insert(policy.tag.clone(), policy);
        }

        for policy in policies {
            for member in &policy.outbounds {
                if in_degree.contains_key(member) {
                    graph
                        .entry(member.clone())
                        .or_default()
                        .push(policy.tag.clone());
                    *in_degree.entry(policy.tag.clone()).or_default() += 1;
                }
            }
        }

        let mut queue = VecDeque::new();
        for policy in policies {
            if in_degree[&policy.tag] == 0 {
                queue.push_back(policy.tag.clone());
            }
        }

        let mut sorted_policies = Vec::new();
        while let Some(tag) = queue.pop_front() {
            sorted_policies.push(policy_map[&tag]);
            if let Some(neighbors) = graph.get(&tag) {
                for neighbor in neighbors {
                    if let Some(deg) = in_degree.get_mut(neighbor) {
                        *deg -= 1;
                        if *deg == 0 {
                            queue.push_back(neighbor.clone());
                        }
                    }
                }
            }
        }

        // Check for cycles: any policy not emitted by the topological sort is part
        // of a cycle. Use a set of sorted tags for O(1) membership instead of O(n²)
        // linear scans of `sorted_policies` per policy.
        let sorted_tags: HashSet<&str> = sorted_policies.iter().map(|p| p.tag.as_str()).collect();
        for policy in policies {
            if !sorted_tags.contains(policy.tag.as_str()) {
                // There is a cycle involving this policy, compile it anyway (it will probably drop members)
                sorted_policies.push(policy);
            }
        }

        for policy in sorted_policies {
            let tag = tag_allocator.allocate_policy_tag(policy);

            let mut runtime_members = Vec::new();

            let mut member_indices = Vec::new();
            for (i, member) in policy.outbounds.iter().enumerate() {
                match self.resolve_target(member, tag_allocator) {
                    Some(tag) if valid_outbounds.contains(&tag) => {
                        if !runtime_members.contains(&tag) {
                            runtime_members.push(tag);
                            member_indices.push(i);
                        }
                    }
                    Some(tag) => {
                        warnings.push(CompileWarning::PolicyMemberIgnored {
                            proxy_policy_tag: policy.tag.clone(),
                            member: member.clone(),
                            reason: format!("Resolved target `{}` is not a valid outbound.", tag),
                        });
                    }
                    None => {
                        warnings.push(CompileWarning::PolicyMemberIgnored {
                            proxy_policy_tag: policy.tag.clone(),
                            member: member.clone(),
                            reason: "Target could not be resolved.".into(),
                        });
                    }
                }
            }

            runtime_members = dedupe(runtime_members);

            if runtime_members.is_empty() {
                warnings.push(CompileWarning::PolicyIgnored {
                    proxy_policy_tag: policy.tag.clone(),
                    name: policy.name.clone(),
                    reason: "Policy has no valid runtime members.".into(),
                });
                runtime_groups.push(RuntimePolicy {
                    tag: tag.clone(),
                    source_proxy_policy_id: policy.tag.clone(),
                    outbounds: Vec::new(),
                    policy: Outbound {
                        tag: tag.clone(),
                        outbound_type: OutboundType::Block,
                    },
                });
                continue;
            }

            let mut runtime_policy = policy.clone();
            runtime_policy.outbounds = runtime_members.clone();
            if let ProxyPolicyType::Balancer(balancer) = &mut runtime_policy.policy_type {
                balancer.weights = balancer.weights.as_ref().map(|weights| {
                    member_indices
                        .iter()
                        .filter_map(|&i| weights.get(i).copied())
                        .collect()
                });
            }

            if let ProxyPolicyType::Manual(manual) = &mut runtime_policy.policy_type {
                manual.default = manual
                    .default
                    .as_ref()
                    .and_then(|default| self.resolve_target(default, tag_allocator))
                    .filter(|default| runtime_members.iter().any(|member| member == default))
                    .or_else(|| runtime_members.first().cloned());
            }

            let outbound = self
                .outbound_factory
                .build_policy_outbound(&runtime_policy, tag.clone())?;

            let allocated_tag = tag.clone();
            runtime_groups.push(RuntimePolicy {
                tag,
                source_proxy_policy_id: policy.tag.clone(),
                outbounds: runtime_members,
                policy: outbound,
            });
            valid_outbounds.insert(allocated_tag);
        }

        Ok(runtime_groups)
    }

    fn compile_rules(
        &self,
        rules: &[RouteRule],
        tag_allocator: &TagAllocator,
        valid_targets: &HashSet<String>,
        warnings: &mut Vec<CompileWarning>,
    ) -> Vec<RouteRule> {
        let mut runtime_rules = Vec::with_capacity(rules.len());

        for (index, rule) in rules.iter().enumerate() {
            let RuleActionType::Route(route_action) = &rule.action.action else {
                runtime_rules.push(rule.clone());
                continue;
            };

            match self.resolve_target(&route_action.outbound, tag_allocator) {
                Some(resolved) if valid_targets.contains(&resolved) => {
                    let mut runtime_rule = rule.clone();
                    if let RuleActionType::Route(route) = &mut runtime_rule.action.action {
                        route.outbound = resolved;
                    }
                    runtime_rules.push(runtime_rule);
                }
                Some(resolved) => {
                    warnings.push(CompileWarning::RuleIgnored {
                        rule_index: index,
                        reason: format!("Resolved target `{}` is not a valid outbound.", resolved),
                    });
                }
                None => {
                    warnings.push(CompileWarning::RuleIgnored {
                        rule_index: index,
                        reason: "Target could not be resolved.".into(),
                    });
                }
            }
        }

        runtime_rules
    }

    fn resolve_proxy_references(&self, server: &mut ProxyServer, tag_allocator: &TagAllocator) {
        let resolve = |target: &mut String| {
            *target = self
                .resolve_target(target, tag_allocator)
                .unwrap_or_else(|| "block".into());
        };
        if let Some(detour) = &mut server.detour {
            resolve(detour);
        }
        let transport = match &mut server.protocol {
            Protocol::Vless(v) => v.transport.as_mut(),
            Protocol::Vmess(v) => v.transport.as_mut(),
            Protocol::Trojan(v) => v.transport.as_mut(),
            _ => None,
        };
        // Resolve only typed references; arbitrary header maps remain opaque.
        if let Some(detour) = transport
            .and_then(|transport| transport.download.as_mut())
            .and_then(|download| download.detour.as_mut())
        {
            resolve(detour);
        }
    }

    fn resolve_target(&self, target: &str, tag_allocator: &TagAllocator) -> Option<String> {
        if target == "direct" || target == "block" || target == "dns-out" {
            return Some(target.to_string());
        }

        if let Some(tag) = tag_allocator.get_proxy_server_tag(target) {
            return Some(tag.to_string());
        }

        if let Some(tag) = tag_allocator.get_policy_tag(target) {
            return Some(tag.to_string());
        }

        if tag_allocator.is_used(target) {
            return Some(target.to_string());
        }

        None
    }

    fn compile_final_outbound(
        &self,
        outbound: &str,
        tag_allocator: &TagAllocator,
        valid_targets: &HashSet<String>,
        warnings: &mut Vec<CompileWarning>,
    ) -> String {
        match self.resolve_target(outbound, tag_allocator) {
            Some(resolved) if valid_targets.contains(&resolved) => resolved,
            Some(resolved) => {
                warnings.push(CompileWarning::FinalPolicyChanged {
                    from: outbound.to_string(),
                    to: "block".to_string(),
                    reason: format!("Resolved target `{resolved}` is not a valid outbound."),
                });
                "block".to_string()
            }
            None => {
                warnings.push(CompileWarning::FinalPolicyChanged {
                    from: outbound.to_string(),
                    to: "block".to_string(),
                    reason: "Target could not be resolved.".into(),
                });
                "block".to_string()
            }
        }
    }

    fn collect_builtin_tags() -> HashSet<String> {
        let mut tags = HashSet::new();
        tags.insert("direct".to_string());
        tags.insert("block".to_string());
        tags.insert("dns-out".to_string());
        tags
    }
}

fn dedupe(mut items: Vec<String>) -> Vec<String> {
    let mut seen = HashSet::new();
    items.retain(|item| {
        if seen.contains(item) {
            false
        } else {
            seen.insert(item.clone());
            true
        }
    });
    items
}
