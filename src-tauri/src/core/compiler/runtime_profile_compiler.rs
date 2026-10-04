use std::collections::{HashMap, HashSet, VecDeque};

use crate::core::{
    compiler::{
        error::CompileError, outbound_factory::OutboundFactory, support_resolver::SupportResolver,
        tag_allocator::TagAllocator, warning::CompileWarning,
    },
    models::{
        policy::{ProxyPolicy, ProxyPolicyType},
        profile::Profile,
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
    pub fn new() -> Self {
        Self {
            support_resolver: SupportResolver,
            outbound_factory: OutboundFactory,
        }
    }

    pub fn compile(&self, profile: &Profile) -> Result<RuntimeProfile, CompileError> {
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

        for server in &regular_servers {
            let (normalized_server, normalize_warning) = self.support_resolver.normalize(server);

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

            let outbound = match self
                .outbound_factory
                .build_proxy_outbound(&normalized_server, tag.clone())
            {
                Ok(outbound) => outbound,
                Err(_) => continue,
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

            if chain_proto.proxies.is_empty() {
                warnings.push(CompileWarning::ProxyServerIgnored {
                    proxy_server_tag: chain_server.tag.clone(),
                    name: chain_server.name.clone(),
                    reason: "Chain contains no proxies.".to_string(),
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

                if i == 0 {
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

                let allocated_tag = tag_allocator.allocate_proxy_server_tag(&cloned_server);

                match self
                    .outbound_factory
                    .build_proxy_outbound(&cloned_server, allocated_tag.clone())
                {
                    Ok(outbound) => {
                        outbounds.push(RuntimeOutbound {
                            tag: allocated_tag.clone(),
                            source_proxy_server_id: cloned_server.tag.clone(),
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
            }
        }

        let mut valid_targets = Self::collect_builtin_tags();
        for outbound in &outbounds {
            valid_targets.insert(outbound.tag.clone());
        }

        let policies = self.compile_policy(
            &profile.proxy_policies,
            &mut tag_allocator,
            &mut valid_targets,
            &mut warnings,
        )?;

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
            live_temp: profile.live_temp.clone(),
            live_permanent: profile.live_permanent.clone(),
            rule_sets: profile.rule_sets.clone(),
            final_outbound,
            dns: profile.dns.clone(),
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
        for (tag, &deg) in &in_degree {
            if deg == 0 {
                queue.push_back(tag.clone());
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

            for member in &policy.outbounds {
                match self.resolve_target(member, tag_allocator) {
                    Some(tag) if valid_outbounds.contains(&tag) => {
                        runtime_members.push(tag);
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
                continue;
            }

            let mut runtime_policy = policy.clone();
            runtime_policy.outbounds = runtime_members.clone();

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
                    to: "direct".to_string(),
                    reason: format!("Resolved target `{resolved}` is not a valid outbound."),
                });
                "direct".to_string()
            }
            None => {
                warnings.push(CompileWarning::FinalPolicyChanged {
                    from: outbound.to_string(),
                    to: "direct".to_string(),
                    reason: "Target could not be resolved.".into(),
                });
                "direct".to_string()
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
