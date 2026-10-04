use std::collections::HashSet;

use crate::core::{
    compiler::tag_allocator::allocate_named_tag,
    dto::{
        Proxy, ProxyOrigin, SubUserinfo, SyncResult, SyncSummary, ZeytunCore,
        DEFAULT_FINAL_OUTBOUND,
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

impl SubscriptionManager {
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
                        rules_orphaned: 0,
                        at_unix_ms: crate::core::constants::unix_millis(),
                        errors: vec![format!("fetch failed: {e}")],
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
        let parsed = Self::parse_proxy_links(raw);
        if parsed.is_empty() {
            return SyncResult {
                summary: SyncSummary {
                    proxies_added: 0,
                    proxies_removed: 0,
                    proxies_kept: 0,
                    rules_orphaned: 0,
                    at_unix_ms: crate::core::constants::unix_millis(),
                    errors: vec!["no proxy links found in subscription response".to_string()],
                },
                // Headers are still useful even when the body has no links.
                userinfo: headers.userinfo,
                update_interval_hours: headers.update_interval_hours,
                profile_title: headers.profile_title,
            };
        }

        let link_normalized: Vec<String> =
            parsed.iter().map(|link| link.trim().to_string()).collect();

        // Diff on a STABLE key derived from the parsed connection params (tag and
        // display name stripped), not the raw link. Subscription servers often
        // reorder query params or change the `#remark` between fetches while the
        // proxy is functionally identical — keying on the raw link then falsely
        // reports "1 added / 1 removed" for an unchanged proxy.
        let existing_keys: HashSet<String> = profile
            .proxies
            .iter()
            .filter(|proxy| proxy.origin == ProxyOrigin::Subscription)
            .map(Self::proxy_stable_key)
            .collect();

        let incoming_keys: HashSet<String> = link_normalized
            .iter()
            .filter_map(|link| Self::link_stable_key(link))
            .collect();

        let mut errors = Vec::new();
        let mut proxies_added = 0;
        let mut proxies_kept = 0;
        let mut used_tags = HashSet::from([
            "direct".to_string(),
            "block".to_string(),
            "dns-out".to_string(),
        ]);
        used_tags.extend(profile.policies.iter().map(|policy| policy.tag.clone()));
        used_tags.extend(profile.proxies.iter().map(|proxy| proxy.tag.clone()));

        for link in &link_normalized {
            let incoming_key = Self::link_stable_key(link);
            if let Some(key) = &incoming_key {
                if existing_keys.contains(key) {
                    proxies_kept += 1;
                    continue;
                }
            }

            let preview = match link_parser::proxy_from_link(
                link,
                ProxyOrigin::Subscription,
                "proxy-preview".to_string(),
            ) {
                Ok(proxy) => proxy,
                Err(e) => {
                    errors.push(format!("failed to parse proxy from subscription: {e}"));
                    continue;
                }
            };
            let tag = allocate_named_tag("proxy", &preview.title, &mut used_tags);

            match link_parser::proxy_from_link(link, ProxyOrigin::Subscription, tag) {
                Ok(proxy) => {
                    profile.proxies.push(proxy);
                    proxies_added += 1;
                }
                Err(e) => {
                    errors.push(format!("failed to parse proxy from subscription: {e}"));
                }
            }
        }

        // Prune stale subscription proxies (present locally, gone upstream) —
        // compared on the same stable key.
        let stale: Vec<String> = profile
            .proxies
            .iter()
            .filter(|proxy| {
                proxy.origin == ProxyOrigin::Subscription
                    && !incoming_keys.contains(&Self::proxy_stable_key(proxy))
            })
            .map(|proxy| proxy.tag.clone())
            .collect();

        let proxies_removed = stale.len();
        let stale_set: HashSet<&String> = stale.iter().collect();

        profile
            .proxies
            .retain(|proxy| !stale_set.contains(&proxy.tag));

        // Clean policy member references to pruned proxies.
        for policy in &mut profile.policies {
            if let Some(members) = policy.members.as_mut() {
                members.retain(|m| !stale_set.contains(m));
            }
        }

        // Option A (silent): repoint any rule targeting a pruned proxy to the
        // default final outbound and flag it orphaned.
        let mut rules_orphaned = 0;
        for rule in &mut profile.rules {
            if stale_set.contains(&rule.outbound) {
                rule.outbound = DEFAULT_FINAL_OUTBOUND.to_string();
                rule.orphaned = true;
                rules_orphaned += 1;
            }
        }

        SyncResult {
            summary: SyncSummary {
                proxies_added,
                proxies_removed,
                proxies_kept,
                rules_orphaned,
                at_unix_ms: crate::core::constants::unix_millis(),
                errors,
            },
            userinfo: headers.userinfo,
            update_interval_hours: headers.update_interval_hours,
            profile_title: headers.profile_title,
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

    /// Stable identity key for an incoming subscription link. `None` when the
    /// link can't be parsed (handled by the caller, which treats it as new).
    fn link_stable_key(link: &str) -> Option<String> {
        link_parser::proxy_from_link(link, ProxyOrigin::Subscription, "k".to_string())
            .ok()
            .and_then(|proxy| link_parser::proxy_to_proxy_server(&proxy).ok())
            .map(|server| Self::server_stable_key(&server))
    }

    /// Canonicalize a `ProxyServer` into a diff key: clear the display-only
    /// `tag`/`name` fields, then serialize. `serde_json::Value` object keys
    /// serialize in a stable (sorted) order via BTreeMap, so param reordering in
    /// the source link does not change the key.
    fn server_stable_key(server: &ProxyServer) -> String {
        let mut normalized = server.clone();
        normalized.tag = String::new();
        normalized.name = String::new();
        match serde_json::to_value(&normalized) {
            Ok(value) => value.to_string(),
            Err(_) => format!("{}:{}", normalized.address, normalized.port),
        }
    }

    /// Fetch a subscription URL, optionally through `proxy` (an `http://…` URL
    /// from [`NetworkPolicy::resolve`]). A malformed proxy URL is an error, not a
    /// silent fall back to direct: the user asked for a specific route and
    /// quietly ignoring it would leak the request onto the wrong path.
    pub fn fetch_url(url: &str, proxy: Option<&str>) -> Result<(String, SubHeaders), String> {
        let mut builder = ureq::AgentBuilder::new()
            // Subscription servers are often behind overloaded VPS frontends; a
            // hung fetch must not wedge the refresh path forever.
            .timeout(std::time::Duration::from_secs(30))
            .timeout_connect(std::time::Duration::from_secs(15));
        if let Some(raw) = proxy {
            let parsed =
                ureq::Proxy::new(raw).map_err(|e| format!("invalid proxy URL `{raw}`: {e}"))?;
            builder = builder.proxy(parsed);
        }

        let response = builder
            .build()
            .get(url)
            .set("User-Agent", "Zeytun/0.1")
            .call()
            .map_err(|e| format!("HTTP request failed: {e}"))?;

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
        const MAX_BODY_BYTES: usize = 10 * 1024 * 1024;
        use std::io::Read;
        let mut raw = Vec::new();
        response
            .into_reader()
            .take((MAX_BODY_BYTES + 1) as u64)
            .read_to_end(&mut raw)
            .map_err(|e| format!("failed to read response body: {e}"))?;
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

    fn parse_proxy_links(raw: &str) -> Vec<String> {
        let trimmed = raw.trim();
        if let Ok(decoded) = Self::try_decode_base64(trimmed) {
            let lines: Vec<String> = decoded
                .lines()
                .map(|l| l.trim().to_string())
                .filter(|l| !l.is_empty() && l.contains("://"))
                .collect();

            if !lines.is_empty() {
                return lines;
            }
        }

        raw.lines()
            .map(|l| l.trim().to_string())
            .filter(|l| !l.is_empty() && l.contains("://"))
            .collect()
    }

    fn try_decode_base64(input: &str) -> Result<String, String> {
        use base64::{engine::general_purpose, Engine as _};

        let normalized = input.trim().trim_end_matches('=');
        let padded = match normalized.len() % 4 {
            0 => normalized.to_string(),
            2 => format!("{normalized}=="),
            3 => format!("{normalized}="),
            _ => normalized.to_string(),
        };

        let bytes = general_purpose::STANDARD
            .decode(&padded)
            .or_else(|_| general_purpose::URL_SAFE.decode(&padded))
            .map_err(|_| "invalid base64".to_string())?;

        String::from_utf8(bytes).map_err(|_| "invalid utf-8 after base64 decode".to_string())
    }
}
