use serde::{Deserialize, Serialize};
use std::collections::HashSet;

use crate::core::dto::{
    DnsConfig, Proxy, ProxyOrigin, ProxyPolicy, ProxyPolicyType, Rule, RuleSet, SubscriptionSource,
    ZeytunCore, DEFAULT_SELECTOR_POLICY_TAG,
};

pub const CURRENT_SCHEMA_VERSION: u32 = 2;
pub const FORMAT_IDENTIFIER: &str = "zeytun-profile";
pub const MAX_DOCUMENT_BYTES: usize = 32 * 1024 * 1024; // 32 MiB
pub const MAX_SINGLE_ASSET_BYTES: usize = 10 * 1024 * 1024; // 10 MiB
pub const MAX_TOTAL_ASSET_BYTES: usize = 20 * 1024 * 1024; // 20 MiB

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExportMode {
    Full,
    Snapshot,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManagedAsset {
    pub id: String,
    pub kind: String,
    pub base64_data: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProfileDocumentData {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dns: Option<DnsConfig>,
    #[serde(default)]
    pub sources: Vec<SubscriptionSource>,
    #[serde(default)]
    pub proxies: Vec<Proxy>,
    #[serde(default)]
    pub policies: Vec<ProxyPolicy>,
    #[serde(default)]
    pub rules: Vec<Rule>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rule_sets: Option<Vec<RuleSet>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProfileDocument {
    pub format: String,
    pub schema_version: u32,
    pub profile: ProfileDocumentData,
    #[serde(default)]
    pub assets: Vec<ManagedAsset>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportPreview {
    pub preview_token: String,
    pub name: String,
    pub mode: ExportMode,
    pub source_count: usize,
    pub proxy_count: usize,
    pub policy_count: usize,
    pub rule_count: usize,
    pub asset_count: usize,
    pub warnings: Vec<String>,
}

impl ProfileDocument {
    pub fn encode(
        name: &str,
        icon: Option<String>,
        profile: &ZeytunCore,
        sources: &[SubscriptionSource],
        mode: ExportMode,
        assets: Vec<ManagedAsset>,
    ) -> Result<String, String> {
        let (doc_sources, doc_proxies) = match mode {
            ExportMode::Full => (sources.to_vec(), profile.proxies.clone()),
            ExportMode::Snapshot => {
                let snapshot_proxies: Vec<Proxy> = profile
                    .proxies
                    .iter()
                    .map(|p| {
                        let mut cloned = p.clone();
                        cloned.origin = ProxyOrigin::Manual;
                        cloned.source_id = None;
                        cloned
                    })
                    .collect();
                (Vec::new(), snapshot_proxies)
            }
        };

        let mut doc_policies = profile.policies.clone();
        if !doc_policies
            .iter()
            .any(|p| p.tag == DEFAULT_SELECTOR_POLICY_TAG)
        {
            let root = ProxyPolicy {
                tag: DEFAULT_SELECTOR_POLICY_TAG.to_string(),
                name: "Global Proxy".to_string(),
                kind: Some(ProxyPolicyType::Selector),
                members: Some(doc_proxies.iter().map(|p| p.tag.clone()).collect()),
                selected_member_tag: doc_proxies.first().map(|p| p.tag.clone()),
                test_url: None,
                interval_seconds: None,
                tolerance_ms: None,
                strategy: None,
                weights: None,
            };
            doc_policies.insert(0, root);
        }

        let mut dns = profile.dns.clone();
        if let Some(dns) = &mut dns {
            if dns.mode.is_some() {
                dns.migrate_legacy();
            }
        }
        let doc = ProfileDocument {
            format: FORMAT_IDENTIFIER.to_string(),
            schema_version: CURRENT_SCHEMA_VERSION,
            profile: ProfileDocumentData {
                name: name.to_string(),
                icon,
                dns,
                sources: doc_sources,
                proxies: doc_proxies,
                policies: doc_policies,
                rules: profile.rules.clone(),
                rule_sets: profile.rule_sets.clone(),
            },
            assets,
        };

        serde_json::to_string_pretty(&doc).map_err(|e| e.to_string())
    }

    pub fn decode(raw: &str) -> Result<Self, String> {
        if raw.len() > MAX_DOCUMENT_BYTES {
            return Err(format!(
                "document size {} exceeds maximum allowed {} bytes",
                raw.len(),
                MAX_DOCUMENT_BYTES
            ));
        }

        let doc: ProfileDocument = serde_json::from_str(raw)
            .map_err(|e| format!("invalid native profile document: {e}"))?;

        if doc.format != FORMAT_IDENTIFIER {
            return Err(format!(
                "unsupported format '{}', expected '{}'",
                doc.format, FORMAT_IDENTIFIER
            ));
        }

        if !(1..=CURRENT_SCHEMA_VERSION).contains(&doc.schema_version) {
            return Err(format!(
                "unsupported schema_version {}, current supported is {}",
                doc.schema_version, CURRENT_SCHEMA_VERSION
            ));
        }

        doc.validate()?;
        Ok(doc)
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.profile.name.trim().is_empty() {
            return Err("profile name must not be empty".into());
        }

        let mut total_asset_bytes = 0;
        for asset in &self.assets {
            let data_bytes = asset.base64_data.len();
            if data_bytes > MAX_SINGLE_ASSET_BYTES {
                return Err(format!(
                    "asset '{}' size exceeds limit of {} bytes",
                    asset.id, MAX_SINGLE_ASSET_BYTES
                ));
            }
            total_asset_bytes += data_bytes;
        }
        if total_asset_bytes > MAX_TOTAL_ASSET_BYTES {
            return Err(format!(
                "total asset size {} exceeds limit of {} bytes",
                total_asset_bytes, MAX_TOTAL_ASSET_BYTES
            ));
        }

        let mut proxy_tags = HashSet::new();
        let builtins = HashSet::from(["direct", "block", "dns-out"]);
        for proxy in &self.profile.proxies {
            if builtins.contains(proxy.tag.as_str()) {
                return Err(format!(
                    "proxy tag '{}' collides with reserved builtin",
                    proxy.tag
                ));
            }
            if !proxy_tags.insert(&proxy.tag) {
                return Err(format!("duplicate proxy tag '{}' in profile", proxy.tag));
            }
        }

        let mut policy_tags = HashSet::new();
        for policy in &self.profile.policies {
            if !policy_tags.insert(&policy.tag) {
                return Err(format!("duplicate policy tag '{}' in profile", policy.tag));
            }
        }

        let mut source_ids = HashSet::new();
        for source in &self.profile.sources {
            if !source_ids.insert(&source.id) {
                return Err(format!("duplicate source id '{}' in profile", source.id));
            }
            source.settings.validate()?;
        }

        for proxy in &self.profile.proxies {
            if proxy.origin == ProxyOrigin::Subscription {
                if let Some(src_id) = &proxy.source_id {
                    if !source_ids.contains(src_id) && !self.profile.sources.is_empty() {
                        return Err(format!(
                            "proxy '{}' references non-existent source '{}'",
                            proxy.tag, src_id
                        ));
                    }
                }
            }
        }

        let (mut profile, _) = self.to_zeytun_core("dns-validation");
        crate::core::dns_config::normalize_profile_dns(&mut profile)?;
        crate::core::manager::validate_profile_dns_rules(&profile).map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn preview(&self) -> ImportPreview {
        let mut warnings = Vec::new();
        let mode = if self.profile.sources.is_empty() {
            ExportMode::Snapshot
        } else {
            ExportMode::Full
        };

        if mode == ExportMode::Full {
            warnings.push("Document contains subscription sources with upstream URLs.".into());
        }

        let preview_token = uuid::Uuid::new_v4().to_string();

        ImportPreview {
            preview_token,
            name: self.profile.name.clone(),
            mode,
            source_count: self.profile.sources.len(),
            proxy_count: self.profile.proxies.len(),
            policy_count: self.profile.policies.len(),
            rule_count: self.profile.rules.len(),
            asset_count: self.assets.len(),
            warnings,
        }
    }

    pub fn to_zeytun_core(&self, new_id: &str) -> (ZeytunCore, Vec<SubscriptionSource>) {
        let mut core = ZeytunCore {
            id: new_id.to_string(),
            dns: Some(
                self.profile
                    .dns
                    .clone()
                    .unwrap_or_else(DnsConfig::with_default_server),
            ),
            proxies: self.profile.proxies.clone(),
            policies: self.profile.policies.clone(),
            rules: self.profile.rules.clone(),
            rule_sets: self.profile.rule_sets.clone(),
            ..ZeytunCore::default()
        };
        core.normalize_policy_graph();
        if self.schema_version == 1 {
            core.dns
                .get_or_insert_with(Default::default)
                .migrate_legacy();
        } else if let Some(dns) = &mut core.dns {
            if dns.mode.is_some() {
                dns.migrate_legacy();
            }
        }
        (core, self.profile.sources.clone())
    }
}
