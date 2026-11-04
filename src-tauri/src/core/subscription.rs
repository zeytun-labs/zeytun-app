use std::collections::{HashMap, HashSet};

use crate::core::{
    compiler::tag_allocator::allocate_named_tag,
    dto::{
        Proxy, ProxyOrigin, SubUserinfo, SubscriptionSource, SyncResult, SyncSummary, ZeytunCore,
        DEFAULT_SELECTOR_POLICY_TAG,
    },
    link_parser,
    models::proxy::ProxyServer,
};

/// Headers Zeytun understands from a subscription response.
#[derive(Default)]
pub struct SubHeaders {
    userinfo: Option<SubUserinfo>,
    update_interval_hours: Option<u32>,
    profile_title: Option<String>,
}

pub struct SubscriptionManager;

pub const MAX_SUBSCRIPTION_BODY_BYTES: usize = 10 * 1024 * 1024;

/// No raw URLs, titles, tokens or parser errors cross this boundary.
#[derive(Debug, serde::Serialize, thiserror::Error)]
#[error("subscription rejected: {kind}; {invalid_count} invalid candidate(s), lines {lines:?}")]
pub struct SubscriptionParseError {
    pub kind: &'static str,
    pub invalid_count: usize,
    pub lines: Vec<usize>,
}

pub struct StagedSubscription {
    nodes: Vec<Proxy>,
    headers: SubHeaders,
}

impl SubscriptionManager {
    pub(crate) fn deletion_dependencies(
        profile: &ZeytunCore,
        source_id: &str,
    ) -> Vec<crate::core::dto::SubscriptionDependency> {
        use crate::core::dto::{
            RuleType, SubscriptionDependency, SubscriptionDependencyKind as Kind,
        };
        let tags = profile
            .proxies
            .iter()
            .filter(|p| p.source_id.as_deref() == Some(source_id))
            .map(|p| p.tag.as_str())
            .collect::<HashSet<_>>();
        let mut refs = Vec::new();
        let mut add = |kind, owner: String, target: &str| {
            if tags.contains(target) {
                refs.push(SubscriptionDependency {
                    kind,
                    owner,
                    proxy_tag: target.into(),
                });
            }
        };
        for policy in &profile.policies {
            // Synthetic root membership and selection are registry-derived, not authored intent.
            if policy.tag != DEFAULT_SELECTOR_POLICY_TAG {
                for tag in policy.members.iter().flatten() {
                    add(Kind::PolicyMember, policy.tag.clone(), tag);
                }
                if let Some(tag) = &policy.selected_member_tag {
                    add(Kind::PolicySelection, policy.tag.clone(), tag);
                }
            }
        }
        for proxy in &profile.proxies {
            if let Ok(server) = link_parser::proxy_to_proxy_server(proxy) {
                let mut detours = HashSet::new();
                Self::server_references(&server, &mut detours);
                let chains = match &server.protocol {
                    crate::core::models::proxy::Protocol::Chain(chain) => chain
                        .proxies
                        .iter()
                        .map(String::as_str)
                        .collect::<HashSet<_>>(),
                    _ => HashSet::new(),
                };
                let mut sorted = detours.into_iter().collect::<Vec<_>>();
                sorted.sort();
                for tag in sorted {
                    add(
                        if chains.contains(tag.as_str()) {
                            Kind::ProxyChain
                        } else {
                            Kind::ProxyDetour
                        },
                        proxy.tag.clone(),
                        &tag,
                    );
                }
            }
        }
        for rule in &profile.rules {
            add(
                if matches!(rule.kind, RuleType::Final) {
                    Kind::Final
                } else {
                    Kind::Route
                },
                rule.id.to_string(),
                &rule.outbound,
            );
        }
        for rule in &profile.temp_rules {
            add(Kind::TempRule, rule.id.to_string(), &rule.outbound);
        }
        for rule in &profile.session_rules {
            add(Kind::SessionRule, rule.id.to_string(), &rule.outbound);
        }
        for server in profile
            .dns
            .iter()
            .flat_map(|dns| dns.servers.iter().flatten())
        {
            if let Some(tag) = server
                .detour
                .as_ref()
                .filter(|tag| tag.as_str() != crate::core::zeytun_core_config::DNS_FOLLOW_MODE)
            {
                add(Kind::DnsDetour, server.tag.clone(), tag);
            }
        }
        for rs in profile.rule_sets.iter().flatten() {
            add(Kind::RulesetAction, rs.tag.clone(), &rs.action);
            if let Some(tag) = &rs.download_policy {
                add(Kind::RulesetDownloadPolicy, rs.tag.clone(), tag);
            }
        }
        refs
    }

    pub(crate) fn runtime_changed(before: &ZeytunCore, after: &ZeytunCore) -> bool {
        let runtime = |p: &ZeytunCore| {
            p.proxies
                .iter()
                .map(|n| {
                    (
                        n.tag.clone(),
                        (n.enabled, n.available, Self::proxy_stable_key(n)),
                    )
                })
                .collect::<HashMap<_, _>>()
        };
        runtime(before) != runtime(after)
    }
    /// Grammar: one supported share link per line, blank lines and lines
    /// beginning with '#' are comments. Everything else rejects the snapshot.
    pub fn stage(
        raw: &str,
        headers: SubHeaders,
    ) -> Result<StagedSubscription, SubscriptionParseError> {
        let error = |kind, invalid_count, lines| SubscriptionParseError {
            kind,
            invalid_count,
            lines,
        };
        if raw.len() > MAX_SUBSCRIPTION_BODY_BYTES {
            return Err(error("body too large", 0, vec![]));
        }
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Err(error("empty body", 0, vec![]));
        }
        let decoded = Self::try_decode_base64(trimmed).ok();
        let text = decoded.as_deref().unwrap_or(trimmed);
        if text.trim_start().starts_with(['<', '{', '['])
            || text.trim_start().starts_with("proxies:")
        {
            return Err(error(
                "unsupported format (plain/base64 share links required)",
                0,
                vec![],
            ));
        }
        let mut nodes = Vec::new();
        let mut invalid_count = 0;
        let mut lines = Vec::new();
        for (index, line) in text.lines().enumerate() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            match link_parser::proxy_from_link(line, ProxyOrigin::Subscription, "staged".into()) {
                Ok(node) => nodes.push(node),
                Err(_) => {
                    invalid_count += 1;
                    if lines.len() < 3 {
                        lines.push(index + 1);
                    }
                }
            }
        }
        if invalid_count > 0 {
            return Err(error(
                "invalid or unsupported share link",
                invalid_count,
                lines,
            ));
        }
        if nodes.is_empty() {
            return Err(error("zero valid nodes", 0, vec![]));
        }
        Ok(StagedSubscription { nodes, headers })
    }

    pub(crate) fn failed_result(error: String) -> SyncResult {
        SyncResult {
            summary: SyncSummary {
                proxies_added: 0,
                proxies_removed: 0,
                proxies_kept: 0,
                proxies_updated: 0,
                rules_orphaned: 0,
                at_unix_ms: crate::core::constants::unix_millis(),
                errors: vec![error],
                effective_fallbacks: Vec::new(),
            },
            userinfo: None,
            update_interval_hours: None,
            profile_title: None,
        }
    }
    /// Fetch a subscription URL and reconcile it into the profile's proxy list.
    /// Only `Subscription`-origin proxies are pruned/replaced; manual proxies
    /// survive. Rules pointing at a pruned proxy are silently repointed to the
    /// default final outbound and flagged `orphaned` (Option A). Returns a
    /// `SyncResult` (summary + parsed subscription headers).
    pub fn fetch_and_update(
        profile: &mut ZeytunCore,
        url: &str,
        proxy: Option<&str>,
    ) -> SyncResult {
        let (raw, headers) = match Self::fetch_url(url, proxy) {
            Ok(pair) => pair,
            Err(e) => {
                return SyncResult {
                    summary: SyncSummary {
                        proxies_added: 0,
                        proxies_removed: 0,
                        proxies_kept: 0,
                        proxies_updated: 0,
                        rules_orphaned: 0,
                        at_unix_ms: crate::core::constants::unix_millis(),
                        errors: vec![format!("fetch failed: {e}")],
                        effective_fallbacks: Vec::new(),
                    },
                    userinfo: None,
                    update_interval_hours: None,
                    profile_title: None,
                };
            }
        };

        Self::apply_update(profile, &raw, headers)
    }

    pub fn apply_update(profile: &mut ZeytunCore, raw: &str, headers: SubHeaders) -> SyncResult {
        let staged = match Self::stage(raw, headers) {
            Ok(staged) => staged,
            Err(error) => return Self::failed_result(error.to_string()),
        };
        // Transitional pure adapter: an empty or multi-source registry cannot
        // identify its owner. Production callers pass the source explicitly.
        let ids = profile
            .proxies
            .iter()
            .filter_map(|p| p.source_id.clone())
            .collect::<HashSet<_>>();
        if ids.len() != 1 {
            return Self::failed_result("single-source adapter requires an explicit source".into());
        }
        Self::reconcile_nodes(profile, ids.iter().next().expect("one source"), staged)
            .unwrap_or_else(Self::failed_result)
    }

    pub fn reconcile(
        profile: &mut ZeytunCore,
        source: &SubscriptionSource,
        staged: StagedSubscription,
    ) -> Result<SyncResult, String> {
        if !source.settings.enabled {
            return Err("subscription updates are paused".into());
        }
        Self::reconcile_nodes(profile, &source.id, staged)
    }

    fn reconcile_nodes(
        profile: &mut ZeytunCore,
        source_id: &str,
        staged: StagedSubscription,
    ) -> Result<SyncResult, String> {
        let mut existing: HashMap<String, Proxy> = HashMap::new();
        for proxy in profile.proxies.iter().filter(|p| {
            p.origin == ProxyOrigin::Subscription && p.source_id.as_deref() == Some(source_id)
        }) {
            existing
                .entry(Self::proxy_stable_key(proxy))
                .or_insert_with(|| proxy.clone());
        }
        let mut used_tags = HashSet::from(["direct".into(), "block".into(), "dns-out".into()]);
        used_tags.extend(profile.policies.iter().map(|p| p.tag.clone()));
        used_tags.extend(profile.proxies.iter().map(|p| p.tag.clone()));
        let mut seen = HashSet::new();
        let mut incoming = Vec::new();
        let mut proxies_added = 0;
        let mut proxies_kept = 0;
        let mut proxies_updated = 0;
        for mut node in staged.nodes {
            let key = Self::proxy_stable_key(&node);
            if !seen.insert(key.clone()) {
                continue;
            }
            if let Some(old) = existing.get(&key) {
                if !old.available
                    || Self::proxy_connection_key(old) != Self::proxy_connection_key(&node)
                {
                    proxies_updated += 1;
                }
                node.tag = old.tag.clone();
                node.enabled = old.enabled;
                proxies_kept += 1;
            } else {
                node.tag = allocate_named_tag("proxy", &node.title, &mut used_tags);
                proxies_added += 1;
            }
            node.source_id = Some(source_id.into());
            node.available = true;
            if let Some(config) = node.config.as_mut() {
                config["tag"] = node.tag.clone().into();
            }
            incoming.push(node);
        }
        let referenced = Self::referenced_proxy_tags(profile);
        let incoming_order = incoming.iter().map(|n| n.tag.clone()).collect::<Vec<_>>();
        let mut incoming_by_tag = incoming
            .into_iter()
            .map(|node| (node.tag.clone(), node))
            .collect::<HashMap<_, _>>();
        let mut proxies_removed = 0;
        let mut candidate = profile.clone();
        candidate.proxies.retain_mut(|node| {
            if node.origin != ProxyOrigin::Subscription
                || node.source_id.as_deref() != Some(source_id)
            {
                return true;
            }
            // Update retained identities in place. Moving them to the end on
            // every refresh reshuffles the root selector and advances the saved
            // revision even when the provider returned the same endpoints.
            if let Some(updated) = incoming_by_tag.remove(&node.tag) {
                *node = updated;
                return true;
            }
            if node.available {
                proxies_removed += 1;
            }
            if referenced.contains(&node.tag) {
                node.available = false;
                true
            } else {
                false
            }
        });
        // Only genuinely new identities are appended, in provider order.
        candidate.proxies.extend(
            incoming_order
                .iter()
                .filter_map(|tag| incoming_by_tag.remove(tag)),
        );
        // The synthetic root follows the unified registry, never a source.
        // Do not normalize authored policies: their selection/order/weights
        // must survive unavailability untouched.
        let root_members = candidate
            .proxies
            .iter()
            .map(|p| p.tag.clone())
            .chain(
                candidate
                    .policies
                    .iter()
                    .filter(|p| p.tag != DEFAULT_SELECTOR_POLICY_TAG)
                    .map(|p| p.tag.clone()),
            )
            .collect::<Vec<_>>();
        if let Some(root) = candidate
            .policies
            .iter_mut()
            .find(|p| p.tag == DEFAULT_SELECTOR_POLICY_TAG)
        {
            root.members = (!root_members.is_empty()).then_some(root_members);
            if root.selected_member_tag.is_none() {
                root.selected_member_tag = root
                    .members
                    .as_ref()
                    .and_then(|members| members.first().cloned());
            }
        }
        let mut effective_fallbacks = Vec::new();
        for policy in &candidate.policies {
            if let Some(selected) = &policy.selected_member_tag {
                let members = policy.members.as_deref().unwrap_or(&[]);
                let valid_members: Vec<&String> = members
                    .iter()
                    .filter(|m| {
                        candidate
                            .proxies
                            .iter()
                            .any(|p| &p.tag == *m && p.available)
                            || candidate.policies.iter().any(|other| &other.tag == *m)
                            || *m == "direct"
                            || *m == "block"
                    })
                    .collect();
                if !valid_members.contains(&selected) {
                    if let Some(fallback) = valid_members.first() {
                        effective_fallbacks
                            .push(format!("{}: {} -> {}", policy.name, selected, fallback));
                    }
                }
            }
        }
        Self::validate_candidate(&candidate)?;
        *profile = candidate;
        Ok(SyncResult {
            summary: SyncSummary {
                proxies_added,
                proxies_removed,
                proxies_kept,
                proxies_updated,
                rules_orphaned: 0,
                at_unix_ms: crate::core::constants::unix_millis(),
                errors: Vec::new(),
                effective_fallbacks,
            },
            userinfo: staged.headers.userinfo,
            update_interval_hours: staged.headers.update_interval_hours,
            profile_title: staged.headers.profile_title,
        })
    }

    pub(crate) fn validate_candidate(profile: &ZeytunCore) -> Result<(), String> {
        let builtins = HashSet::from([
            "direct".to_string(),
            "block".to_string(),
            "dns-out".to_string(),
        ]);
        let mut graph: HashMap<String, HashSet<String>> = HashMap::new();
        for policy in &profile.policies {
            let members = policy.members.as_deref().unwrap_or(&[]);
            if policy
                .weights
                .as_ref()
                .is_some_and(|weights| weights.len() != members.len())
            {
                return Err("policy weights must match authored member count".into());
            }
            if policy
                .selected_member_tag
                .as_ref()
                .is_some_and(|tag| !members.contains(tag))
            {
                return Err("policy selection must reference an authored member".into());
            }
            if builtins.contains(&policy.tag)
                || graph
                    .insert(policy.tag.clone(), members.iter().cloned().collect())
                    .is_some()
            {
                return Err("duplicate or reserved graph tag".into());
            }
        }
        for proxy in &profile.proxies {
            let mut refs = HashSet::new();
            Self::proxy_references(proxy, &mut refs);
            if builtins.contains(&proxy.tag)
                || proxy.tag == DEFAULT_SELECTOR_POLICY_TAG
                || graph.insert(proxy.tag.clone(), refs).is_some()
            {
                return Err("duplicate or reserved graph tag".into());
            }
        }
        let known = graph
            .keys()
            .cloned()
            .chain(builtins)
            .chain(["reject".into(), "hijack-dns".into(), "bypass".into()])
            .collect::<HashSet<_>>();
        if Self::referenced_proxy_tags(profile)
            .iter()
            .any(|tag| !known.contains(tag))
            || graph.values().flatten().any(|tag| !known.contains(tag))
        {
            return Err("candidate graph contains an unknown target".into());
        }
        // Kahn's algorithm avoids recursive stack growth on large subscriptions.
        let mut remaining = graph
            .iter()
            .map(|(tag, refs)| {
                (
                    tag.clone(),
                    refs.iter().filter(|r| graph.contains_key(*r)).count(),
                )
            })
            .collect::<HashMap<_, _>>();
        let mut dependents: HashMap<&str, Vec<&str>> = HashMap::new();
        for (tag, refs) in &graph {
            for reference in refs {
                dependents.entry(reference).or_default().push(tag);
            }
        }
        let mut ready = remaining
            .iter()
            .filter(|(_, count)| **count == 0)
            .map(|(tag, _)| tag.clone())
            .collect::<Vec<_>>();
        let mut visited = 0;
        while let Some(tag) = ready.pop() {
            visited += 1;
            for dependent in dependents.get(tag.as_str()).into_iter().flatten() {
                let count = remaining.get_mut(*dependent).expect("known graph node");
                *count -= 1;
                if *count == 0 {
                    ready.push((*dependent).to_string());
                }
            }
        }
        if visited != graph.len() {
            return Err("candidate graph contains a policy/proxy dependency cycle".into());
        }
        Ok(())
    }

    fn referenced_proxy_tags(profile: &ZeytunCore) -> HashSet<String> {
        let mut tags = HashSet::new();
        for policy in &profile.policies {
            if policy.tag != DEFAULT_SELECTOR_POLICY_TAG {
                tags.extend(policy.members.iter().flatten().cloned());
            }
            tags.extend(policy.selected_member_tag.iter().cloned());
        }
        tags.extend(profile.rules.iter().map(|r| r.outbound.clone()));
        tags.extend(
            profile
                .temp_rules
                .iter()
                .chain(&profile.session_rules)
                .map(|r| r.outbound.clone()),
        );
        if let Some(dns) = &profile.dns {
            tags.extend(
                dns.servers
                    .iter()
                    .flatten()
                    .filter_map(|s| s.detour.clone())
                    .filter(|tag| tag != crate::core::zeytun_core_config::DNS_FOLLOW_MODE),
            );
        }
        for ruleset in profile.rule_sets.iter().flatten() {
            tags.insert(ruleset.action.clone());
            tags.extend(ruleset.download_policy.iter().cloned());
        }
        for proxy in &profile.proxies {
            Self::proxy_references(proxy, &mut tags);
        }
        tags
    }

    fn proxy_references(proxy: &Proxy, tags: &mut HashSet<String>) {
        // Use the same config-or-share-link parse path as runtime conversion.
        if let Ok(server) = link_parser::proxy_to_proxy_server(proxy) {
            Self::server_references(&server, tags);
        }
    }

    pub(crate) fn server_references(server: &ProxyServer, tags: &mut HashSet<String>) {
        use crate::core::models::proxy::Protocol;
        tags.extend(server.detour.iter().cloned());
        let transport = match &server.protocol {
            Protocol::Chain(chain) => {
                tags.extend(chain.proxies.iter().cloned());
                None
            }
            Protocol::Vless(v) => v.transport.as_ref(),
            Protocol::Vmess(v) => v.transport.as_ref(),
            Protocol::Trojan(v) => v.transport.as_ref(),
            _ => None,
        };
        // The shared crate owns transport/downloadSettings; header maps are opaque.
        if let Some(download) = transport.and_then(|t| t.download.as_ref()) {
            tags.extend(download.detour.iter().cloned());
        }
    }

    /// Stable identity key for a stored proxy: the parsed connection params
    /// (`tag`/`name` cleared) serialized canonically. Two proxies that connect
    /// to the same endpoint with the same settings yield the same key even if
    /// their display name or raw link text differs.
    fn proxy_stable_key(proxy: &Proxy) -> String {
        match link_parser::proxy_to_proxy_server(proxy) {
            Ok(server) => Self::server_stable_key(&server),
            // Unparseable — fall back to the raw link so it still diffs against
            // itself deterministically.
            Err(_) => format!("raw:{}", proxy.link.trim()),
        }
    }

    /// Canonicalize a `ProxyServer` into a diff key: clear the display-only
    /// `tag`/`name` fields, then serialize. `serde_json::Value` object keys
    /// serialize in a stable (sorted) order via BTreeMap, so param reordering in
    /// the source link does not change the key.
    ///
    /// For Reality TLS, servers often rotate ephemeral shortIds (`reality_sid`)
    /// and decoy server names (`sni`) on every subscription download. We normalize
    /// these volatile fields so upstream rotation does not churn node identity.
    fn server_stable_key(server: &ProxyServer) -> String {
        let mut normalized = server.clone();
        Self::normalize_tls_for_key(&mut normalized.protocol);
        Self::server_connection_key(normalized)
    }

    /// Compare connection parameters separately from identity. Rotated Reality
    /// credentials retain their node tag but still need applying to the core.
    fn proxy_connection_key(proxy: &Proxy) -> String {
        match link_parser::proxy_to_proxy_server(proxy) {
            Ok(server) => Self::server_connection_key(server),
            Err(_) => format!("raw:{}", proxy.link.trim()),
        }
    }

    fn server_connection_key(mut server: ProxyServer) -> String {
        server.tag.clear();
        server.name.clear();
        match serde_json::to_value(&server) {
            Ok(value) => value.to_string(),
            Err(_) => format!("{}:{}", server.address, server.port),
        }
    }

    pub(crate) fn normalize_tls_for_key(protocol: &mut crate::core::models::proxy::Protocol) {
        use crate::core::models::proxy::Protocol;
        let tls_opt = match protocol {
            Protocol::Vless(p) => p.tls.as_mut(),
            Protocol::Vmess(p) => p.tls.as_mut(),
            Protocol::Trojan(p) => p.tls.as_mut(),
            Protocol::Tuic(p) => Some(&mut p.tls),
            Protocol::Hysteria2(p) => Some(&mut p.tls),
            _ => None,
        };
        if let Some(tls) = tls_opt {
            if tls.reality_pbk.is_some() {
                tls.reality_sid = None;
                tls.sni = None;
            }
        }
    }

    /// Fetch a subscription URL, optionally through `proxy` (an `http://…` URL
    /// from [`NetworkPolicy::resolve`]). A malformed proxy URL is an error, not a
    /// silent fall back to direct: the user asked for a specific route and
    /// quietly ignoring it would leak the request onto the wrong path.
    pub fn fetch_url(url: &str, proxy: Option<&str>) -> Result<(String, SubHeaders), String> {
        crate::core::dto::validate_subscription_url(url)?;
        let mut builder = ureq::AgentBuilder::new()
            // Subscription servers are often behind overloaded VPS frontends; a
            // hung fetch must not wedge the refresh path forever.
            .timeout(std::time::Duration::from_secs(15))
            .timeout_connect(std::time::Duration::from_secs(8));
        if let Some(raw) = proxy {
            let parsed =
                ureq::Proxy::new(raw).map_err(|_| "invalid download proxy URL".to_string())?;
            builder = builder.proxy(parsed);
        }

        let response = builder
            .build()
            .get(url)
            .set("User-Agent", "Zeytun/0.1")
            .call()
            .map_err(|e| match e {
                ureq::Error::Status(status, _) => format!("HTTP {status}"),
                ureq::Error::Transport(_) => "subscription HTTP transport failed".to_string(),
            })?;

        let status = response.status();
        if status != 200 {
            return Err(format!("HTTP {status}"));
        }

        let headers = SubHeaders {
            userinfo: response
                .header("subscription-userinfo")
                .and_then(Self::parse_userinfo),
            // `Profile-Update-Interval` is advertised in hours.
            update_interval_hours: response
                .header("profile-update-interval")
                .and_then(|v| v.trim().parse::<u32>().ok())
                .filter(|h| *h > 0),
            profile_title: response
                .header("profile-title")
                .map(|v| v.trim().to_string())
                .filter(|v| !v.is_empty()),
        };

        // Cap the body while streaming: a hostile subscription URL must not be
        // able to pin memory, and ureq's `into_string()` silently truncates at
        // 10 MB (which would corrupt a subscription) — so read bounded ourselves.
        const MAX_BODY_BYTES: usize = MAX_SUBSCRIPTION_BODY_BYTES;
        use std::io::Read;
        let mut raw = Vec::new();
        response
            .into_reader()
            .take((MAX_BODY_BYTES + 1) as u64)
            .read_to_end(&mut raw)
            .map_err(|_| "failed to read subscription body".to_string())?;
        if raw.len() > MAX_BODY_BYTES {
            return Err(format!("subscription too large: >{MAX_BODY_BYTES} bytes"));
        }
        let body = String::from_utf8(raw)
            .map_err(|_| "subscription body is not valid UTF-8".to_string())?;

        Ok((body, headers))
    }

    /// Parse `subscription-userinfo: upload=..; download=..; total=..; expire=..`.
    /// Missing fields default to 0 (matching the unlimited/never conventions).
    fn parse_userinfo(raw: &str) -> Option<SubUserinfo> {
        let mut info = SubUserinfo {
            upload: 0,
            download: 0,
            total: 0,
            expire: 0,
        };
        let mut saw_any = false;
        for part in raw.split(';') {
            let Some((key, val)) = part.split_once('=') else {
                continue;
            };
            let Ok(n) = val.trim().parse::<i64>() else {
                continue;
            };
            match key.trim().to_ascii_lowercase().as_str() {
                "upload" => info.upload = n,
                "download" => info.download = n,
                "total" => info.total = n,
                "expire" => info.expire = n,
                _ => continue,
            }
            saw_any = true;
        }
        saw_any.then_some(info)
    }

    fn try_decode_base64(input: &str) -> Result<String, String> {
        use base64::{engine::general_purpose, Engine as _};

        let normalized = input
            .chars()
            .filter(|c| !c.is_ascii_whitespace())
            .collect::<String>();

        let bytes = general_purpose::STANDARD
            .decode(&normalized)
            .or_else(|_| general_purpose::STANDARD_NO_PAD.decode(&normalized))
            .or_else(|_| general_purpose::URL_SAFE.decode(&normalized))
            .or_else(|_| general_purpose::URL_SAFE_NO_PAD.decode(&normalized))
            .map_err(|_| "invalid base64".to_string())?;

        String::from_utf8(bytes).map_err(|_| "invalid utf-8 after base64 decode".to_string())
    }
}
