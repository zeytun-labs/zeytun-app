use std::collections::HashSet;

use serde::{Deserialize, Serialize};

pub const DEFAULT_SELECTOR_POLICY_TAG: &str = "root-policy";
pub const DEFAULT_PROFILE_ID: &str = "default";
pub const FINAL_RULE_ID: u64 = 0;
pub const DEFAULT_FINAL_OUTBOUND: &str = "direct";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OutboundMode {
    Direct,
    Global,
    Rule,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RuleType {
    #[serde(rename = "FINAL")]
    Final,
    #[serde(rename = "DOMAIN")]
    Domain,
    #[serde(rename = "DOMAIN-SUFFIX")]
    DomainSuffix,
    #[serde(rename = "DOMAIN-KEYWORD")]
    DomainKeyword,
    #[serde(rename = "DOMAIN-REGEX")]
    DomainRegex,
    #[serde(rename = "IP-CIDR")]
    IpCidr,
    #[serde(rename = "IN-PORT")]
    InPort,
    #[serde(rename = "DEST-PORT")]
    DestPort,
    #[serde(rename = "GEOIP")]
    Geoip,
    #[serde(rename = "GEOSITE")]
    Geosite,
    #[serde(rename = "PROCESS-NAME")]
    ProcessName,
    #[serde(rename = "PROCESS-PATH")]
    ProcessPath,
    #[serde(rename = "PROCESS-PATH-REGEX")]
    ProcessPathRegex,
    #[serde(rename = "PROTOCOL")]
    Protocol,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum InboundMode {
    Mixed,
    Tun,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProxyEngine {
    ZeytunCore,
    Unsupported,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProxyPolicyType {
    Selector, // manual
    Urltest,  // auto
    Balancer, // balancer
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalProxyConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode: Option<InboundMode>,
    pub listen: String,
    pub socks_port: u16,
    pub http_port: u16,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mixed_port: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tun_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tun_mtu: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tun_auto_route: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub system_proxy: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub log_level: Option<String>,
    /// Hold unmatched TCP and ask UI for outbound (timeout → FINAL).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub connection_ask: Option<ConnectionAskConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionAskConfig {
    #[serde(default)]
    pub enabled: bool,
    /// Milliseconds; default 60000.
    #[serde(default = "default_ask_timeout_ms")]
    pub timeout_ms: u32,
    /// process | process_dest
    #[serde(default = "default_ask_group_by")]
    pub group_by: String,
    /// Prefill Remember checkbox in UI.
    #[serde(default = "default_true")]
    pub remember_default: bool,
}

fn default_ask_timeout_ms() -> u32 {
    60_000
}
fn default_ask_group_by() -> String {
    "process".to_string()
}
fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyPolicy {
    pub tag: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<ProxyPolicyType>, // manual, auto, fallback, balancer
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub members: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selected_member_tag: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub test_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub interval_seconds: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tolerance_ms: Option<u32>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub strategy: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub weights: Option<Vec<u32>>,
}

/// Where a proxy came from. Subscription proxies are pruned/replaced on sync;
/// manual proxies are user-owned and survive syncs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[derive(Default)]
pub enum ProxyOrigin {
    Subscription,
    #[default]
    Manual,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Proxy {
    pub tag: String,
    #[serde(default)]
    pub origin: ProxyOrigin,
    pub title: String,
    pub protocol: String,
    pub link: String,
    pub enabled: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transport: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub config: Option<serde_json::Value>,
}

/// Result of a subscription sync: what changed and any parse/fetch errors.
/// Returned to the caller on foreground sync, persisted on the profile on
/// background sync (with `unread_summary`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncSummary {
    pub proxies_added: usize,
    pub proxies_removed: usize,
    pub proxies_kept: usize,
    pub rules_orphaned: usize,
    pub at_unix_ms: i64,
    #[serde(default)]
    pub errors: Vec<String>,
}

fn default_update_interval_hours() -> u32 {
    12
}

/// Subscription usage/quota parsed from the `subscription-userinfo` response
/// header. Convention: `total == 0` means unlimited, `expire == 0` means never
/// expires. Byte counts and `expire` (unix seconds) come straight from the
/// header.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct SubUserinfo {
    pub upload: i64,
    pub download: i64,
    pub total: i64,
    pub expire: i64,
}

/// Full result of a subscription sync: the reconciliation summary plus metadata
/// parsed from response headers (usage, server-suggested update interval, and
/// the profile title for name fallback).
#[derive(Debug, Clone)]
pub struct SyncResult {
    pub summary: SyncSummary,
    pub userinfo: Option<SubUserinfo>,
    pub update_interval_hours: Option<u32>,
    pub profile_title: Option<String>,
}

/// Registry entry describing a profile. The routing bundle itself is a
/// `ZeytunCore` loaded on demand by id.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileMeta {
    pub id: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subscription_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_updated_unix_ms: Option<i64>,
    pub is_active: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_sync_summary: Option<SyncSummary>,
    #[serde(default)]
    pub unread_summary: bool,

    /// When true, the background auto-updater skips this profile.
    #[serde(default)]
    pub skip_auto_update: bool,
    /// Hours between background auto-updates. Overridden by the subscription's
    /// `Profile-Update-Interval` header when present.
    #[serde(default = "default_update_interval_hours")]
    pub update_interval_hours: u32,

    // Subscription usage from `subscription-userinfo` (None until first sync).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sub_upload: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sub_download: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sub_total: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sub_expire: Option<i64>,

    /// Name of a hugeicons icon (from a fixed frontend-defined set) the user
    /// picked for this profile. None falls back to an initial-letter avatar.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DnsServer {
    pub tag: String,
    pub address: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detour: Option<String>,
    pub name: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DnsConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub servers: Option<Vec<DnsServer>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub final_server: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fake_ip: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rules: Option<Vec<DnsRule>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hosts: Option<Vec<DnsHostEntry>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DnsHostEntry {
    pub id: String,
    pub domain: String,
    pub address: String,
    pub enabled: bool,
}

/// A user-authored DNS routing rule. Maps a domain criterion or an enabled
/// rule-set to a configured DNS server tag or `block` (reject).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DnsRule {
    pub id: String,
    /// "domain" | "domain_suffix" | "ruleset"
    pub kind: String,
    /// A domain, domain suffix, or enabled rule-set tag, depending on `kind`.
    pub value: String,
    /// Target: a DNS server tag, or "block"
    pub target: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleSet {
    pub id: String,
    pub tag: String,
    #[serde(rename = "type")]
    pub kind: String, // "local" or "remote"
    pub source: String,
    pub action: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
    /// Policy tag used as http_client detour when downloading remote ruleset.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub download_policy: Option<String>,
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Human-readable label for this ruleset (shown in UI selectors).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rule {
    pub id: u64,
    pub kind: RuleType,
    pub value: String,
    pub outbound: String,
    pub comment: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rule_set: Option<String>,
    /// Set when the rule's `outbound` target was deleted and repointed to the
    /// default final outbound (Option A). UI surfaces this as a warning.
    #[serde(default)]
    pub orphaned: bool,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

/// Same match fields as `Rule`, plus absolute expiry (unix ms).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TempRule {
    pub id: u64,
    pub kind: RuleType,
    pub value: String,
    pub outbound: String,
    pub comment: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rule_set: Option<String>,
    #[serde(default)]
    pub orphaned: bool,
    /// Unix milliseconds; rule is dropped when `now >= expires_at`.
    pub expires_at: i64,
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Session rule: no expiry (`expires_at` = 0), lives only in app memory,
    /// dies on core reload / profile switch. Never persisted, never baked.
    #[serde(default)]
    pub session: bool,
    /// For session rules created by connection-ask: the group key used in the
    /// core's ask session cache. When this rule is deleted or edited, the key
    /// is sent to `POST /connection-ask/forget` so the cache is invalidated.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ask_group_key: Option<String>,
}

impl TempRule {
    pub fn to_rule(&self) -> Rule {
        Rule {
            id: self.id,
            kind: self.kind,
            value: self.value.clone(),
            outbound: self.outbound.clone(),
            comment: self.comment.clone(),
            rule_set: self.rule_set.clone(),
            orphaned: self.orphaned,
            enabled: self.enabled,
        }
    }

    pub fn is_expired(&self, now_ms: i64) -> bool {
        self.expires_at > 0 && now_ms >= self.expires_at
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZeytunCore {
    #[serde(default)]
    pub id: String,
    pub version: String,
    pub outbound_mode: OutboundMode,
    pub local_proxy: LocalProxyConfig,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dns: Option<DnsConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rule_sets: Option<Vec<RuleSet>>,
    pub policies: Vec<ProxyPolicy>,
    pub proxies: Vec<Proxy>,
    pub rules: Vec<Rule>,
    #[serde(default)]
    pub temp_rules: Vec<TempRule>,
    /// Volatile session rules (ask-remember, "Session" temp type). In-memory
    /// only — never persisted by storage, never baked into the core config;
    /// cleared on core start/reload and on profile switch.
    #[serde(default)]
    pub session_rules: Vec<TempRule>,
}

impl Default for ZeytunCore {
    fn default() -> Self {
        Self {
            id: DEFAULT_PROFILE_ID.to_string(),
            version: "0.0.1".to_string(),
            outbound_mode: OutboundMode::Rule,
            local_proxy: LocalProxyConfig {
                mode: Some(InboundMode::Mixed),
                listen: "127.0.0.1".to_string(),
                socks_port: 6061,
                http_port: 6062,
                mixed_port: Some(6060),
                tun_name: Some("utun9".to_string()),
                tun_mtu: Some(9000),
                tun_auto_route: Some(true),
                system_proxy: Some(false),
                log_level: Some("info".to_string()),
                connection_ask: None,
            },
            dns: None,
            rule_sets: None,
            policies: vec![Self::default_root_policy(
                DEFAULT_SELECTOR_POLICY_TAG,
                "Default",
            )],
            proxies: Vec::new(),
            rules: vec![Self::default_final_rule(DEFAULT_FINAL_OUTBOUND)],
            temp_rules: Vec::new(),
            session_rules: Vec::new(),
        }
    }
}

impl ZeytunCore {
    pub fn normalize_policy_graph(&mut self) {
        if self.policies.is_empty() {
            self.policies.push(Self::default_root_policy(
                DEFAULT_SELECTOR_POLICY_TAG,
                "ROOT_POLICY",
            ));
        }

        if !self
            .policies
            .iter()
            .any(|policy| policy.tag == DEFAULT_SELECTOR_POLICY_TAG)
        {
            self.policies.insert(
                0,
                Self::default_root_policy(DEFAULT_SELECTOR_POLICY_TAG, "ROOT_POLICY"),
            );
        }

        for policy in &mut self.policies {
            if policy.kind.is_none() {
                policy.kind = Some(ProxyPolicyType::Selector);
            }
        }

        let known_policy_tags = self
            .policies
            .iter()
            .map(|policy| policy.tag.clone())
            .collect::<HashSet<_>>();
        let known_proxy_tags = self
            .proxies
            .iter()
            .map(|proxy| proxy.tag.clone())
            .collect::<HashSet<_>>();

        let known_target_tags = known_policy_tags
            .union(&known_proxy_tags)
            .cloned()
            .collect::<HashSet<_>>();

        for policy in &mut self.policies {
            if let Some(members) = policy.members.as_mut() {
                members
                    .retain(|member| known_target_tags.contains(member) && member != &policy.tag);
                dedupe_in_place(members);
                if members.is_empty() {
                    policy.members = None;
                }
            }
            if let Some(selected) = policy.selected_member_tag.as_ref() {
                if !policy
                    .members
                    .as_ref()
                    .map(|members| members.iter().any(|member| member == selected))
                    .unwrap_or(false)
                {
                    policy.selected_member_tag = None;
                }
            }
        }

        let default_members = self
            .proxies
            .iter()
            .map(|proxy| proxy.tag.clone())
            .chain(
                self.policies
                    .iter()
                    .filter(|policy| policy.tag != DEFAULT_SELECTOR_POLICY_TAG)
                    .map(|policy| policy.tag.clone()),
            )
            .collect::<Vec<_>>();

        let default_members = dedupe(default_members);
        if let Some(default_policy) = self
            .policies
            .iter_mut()
            .find(|policy| policy.tag == DEFAULT_SELECTOR_POLICY_TAG)
        {
            default_policy.name = "ROOT_POLICY".to_string();
            default_policy.kind = Some(ProxyPolicyType::Selector);
            default_policy.members = (!default_members.is_empty()).then_some(default_members);
            ensure_selected_policy_member(default_policy);
        }

        for policy in &mut self.policies {
            if matches!(policy.kind, Some(ProxyPolicyType::Selector)) {
                ensure_selected_policy_member(policy);
            }
        }

        self.normalize_route_rules();
    }

    pub fn final_outbound(&self) -> String {
        self.rules
            .iter()
            .rev()
            .find(|rule| matches!(rule.kind, RuleType::Final) || rule.id == FINAL_RULE_ID)
            .map(|rule| rule.outbound.clone())
            .filter(|outbound| !outbound.trim().is_empty())
            .unwrap_or_else(|| DEFAULT_FINAL_OUTBOUND.to_string())
    }

    fn normalize_route_rules(&mut self) {
        let final_outbound = self.final_outbound();
        self.rules
            .retain(|rule| !matches!(rule.kind, RuleType::Final) && rule.id != FINAL_RULE_ID);
        self.rules.push(Self::default_final_rule(&final_outbound));
    }

    fn default_final_rule(outbound: &str) -> Rule {
        Rule {
            id: FINAL_RULE_ID,
            kind: RuleType::Final,
            value: String::new(),
            outbound: outbound.to_string(),
            comment: "Default final rule".to_string(),
            rule_set: None,
            orphaned: false,
            enabled: true,
        }
    }

    fn default_root_policy(tag: &str, name: &str) -> ProxyPolicy {
        ProxyPolicy {
            tag: tag.to_string(),
            name: name.to_string(),
            kind: Some(ProxyPolicyType::Selector),
            members: Some(vec!["direct".to_string()]),
            selected_member_tag: None,
            test_url: None,
            interval_seconds: None,
            tolerance_ms: None,
            strategy: None,
            weights: None,
        }
    }
}

fn ensure_selected_policy_member(policy: &mut ProxyPolicy) {
    let Some(members) = policy
        .members
        .as_ref()
        .filter(|members| !members.is_empty())
    else {
        policy.selected_member_tag = None;
        return;
    };

    let selected_is_valid = policy
        .selected_member_tag
        .as_ref()
        .map(|selected| members.iter().any(|member| member == selected))
        .unwrap_or(false);

    if !selected_is_valid {
        policy.selected_member_tag = members.first().cloned();
    }
}

fn dedupe(items: Vec<String>) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut output = Vec::new();
    for item in items {
        if seen.insert(item.clone()) {
            output.push(item);
        }
    }
    output
}

fn dedupe_in_place(items: &mut Vec<String>) {
    let mut seen = HashSet::new();
    items.retain(|item| seen.insert(item.clone()));
}
