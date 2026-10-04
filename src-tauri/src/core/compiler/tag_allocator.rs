use std::collections::{HashMap, HashSet};

use crate::core::models::{policy::ProxyPolicy, proxy::ProxyServer};

pub struct TagAllocator {
    used: HashSet<String>,

    proxy_server_tags: HashMap<String, String>,
    policy_tags: HashMap<String, String>,
}

impl Default for TagAllocator {
    fn default() -> Self {
        Self::new()
    }
}

impl TagAllocator {
    pub fn new() -> Self {
        let mut used = HashSet::new();

        // built-in tags
        used.insert("direct".into());
        used.insert("block".into());
        used.insert("dns-out".into());

        Self {
            used,
            proxy_server_tags: HashMap::new(),
            policy_tags: HashMap::new(),
        }
    }

    pub fn allocate_proxy_server_tag(&mut self, server: &ProxyServer) -> String {
        if let Some(tag) = self.proxy_server_tags.get(&server.tag) {
            return tag.clone();
        }

        let preferred = server.tag.trim();
        let base = if preferred.is_empty() {
            format!("proxy-{}", sanitize_tag(&server.name))
        } else {
            preferred.to_string()
        };
        let tag = allocate_unique_tag(base, &mut self.used);

        self.proxy_server_tags
            .insert(server.tag.clone(), tag.clone());

        tag
    }

    pub fn allocate_policy_tag(&mut self, policy: &ProxyPolicy) -> String {
        if let Some(tag) = self.policy_tags.get(&policy.tag) {
            return tag.clone();
        }

        let tag = if policy.tag == crate::core::dto::DEFAULT_SELECTOR_POLICY_TAG {
            allocate_unique_tag(
                crate::core::dto::DEFAULT_SELECTOR_POLICY_TAG.to_string(),
                &mut self.used,
            )
        } else {
            let preferred = policy.tag.trim();
            let base = if preferred.is_empty() {
                format!("policy-{}", sanitize_tag(&policy.name))
            } else {
                preferred.to_string()
            };
            allocate_unique_tag(base, &mut self.used)
        };

        self.policy_tags.insert(policy.tag.clone(), tag.clone());

        tag
    }

    pub fn get_proxy_server_tag(&self, proxy_server_id: &str) -> Option<&str> {
        self.proxy_server_tags
            .get(proxy_server_id)
            .map(|value| value.as_str())
    }

    pub fn get_policy_tag(&self, policy_id: &str) -> Option<&str> {
        self.policy_tags.get(policy_id).map(|value| value.as_str())
    }

    pub fn is_used(&self, tag: &str) -> bool {
        self.used.contains(tag)
    }
}

pub fn allocate_named_tag(prefix: &str, name: &str, used: &mut HashSet<String>) -> String {
    let stem = sanitize_tag(name);
    let base = if stem.is_empty() {
        prefix.to_string()
    } else {
        format!("{prefix}-{stem}")
    };
    allocate_unique_tag(base, used)
}

fn allocate_unique_tag(base: String, used: &mut HashSet<String>) -> String {
    let normalized = if base.is_empty() {
        "untitled".to_string()
    } else {
        base
    };

    if !used.contains(&normalized) {
        used.insert(normalized.clone());
        return normalized;
    }

    let mut index = 2;

    loop {
        let candidate = format!("{}-{}", normalized, index);
        if !used.contains(&candidate) {
            used.insert(candidate.clone());
            return candidate;
        }
        index += 1;
    }
}

fn sanitize_tag(input: &str) -> String {
    let lower = input.trim().to_ascii_lowercase();

    let mut output = String::new();
    let mut last_was_dash = false;

    for ch in lower.chars() {
        let mapped = if ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' {
            ch
        } else {
            '-'
        };

        if mapped == '-' {
            if !last_was_dash {
                output.push(mapped);
            }
            last_was_dash = true;
        } else {
            output.push(mapped);
            last_was_dash = false;
        }
    }

    output.trim_matches('-').to_string()
}
