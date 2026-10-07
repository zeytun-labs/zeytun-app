use serde::{Deserialize, Serialize};

use super::{SubUserinfo, SyncSummary};

/// Full replacement: omitted/null settings are invalid, never implicit clears.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SubscriptionSettings {
    pub name: String,
    pub url: String,
    /// Pause fetching only; cached proxies remain usable.
    pub enabled: bool,
    pub auto_update: bool,
    pub update_interval_hours: u32,
}

impl SubscriptionSettings {
    pub fn validate(&self) -> Result<(), String> {
        if self.name.trim().is_empty() {
            return Err("subscription name must not be empty".into());
        }
        validate_subscription_url(&self.url)?;
        if self.update_interval_hours == 0 {
            return Err("subscription interval must be positive".into());
        }
        Ok(())
    }
}

pub fn validate_subscription_url(raw: &str) -> Result<(), String> {
    let invalid = || "subscription URL must be HTTP(S) with a valid host and port".to_string();
    if raw.trim() != raw || raw.chars().any(char::is_control) {
        return Err(invalid());
    }
    let parsed = url::Url::parse(raw).map_err(|_| invalid())?;
    if !matches!(parsed.scheme(), "http" | "https") || parsed.host_str().is_none() {
        return Err(invalid());
    }
    // url normalizes explicit port 0; check the authority before normalization.
    let authority = raw
        .split_once("://")
        .ok_or_else(invalid)?
        .1
        .split(['/', '?', '#'])
        .next()
        .unwrap_or("")
        .rsplit('@')
        .next()
        .unwrap_or("");
    if authority
        .rsplit_once(':')
        .is_some_and(|(_, port)| port.parse::<u16>() == Ok(0))
    {
        return Err(invalid());
    }
    Ok(())
}

/// Scoped by the containing profile; source ids never enter policy schemas.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SubscriptionSource {
    pub id: String,
    pub settings: SubscriptionSettings,
    pub generation: u64,
    pub position: i64,
    #[serde(default)]
    pub provider_title: Option<String>,
    #[serde(default)]
    pub suggested_interval_hours: Option<u32>,
    #[serde(default)]
    pub userinfo: Option<SubUserinfo>,
    #[serde(default)]
    pub last_attempt_at: Option<i64>,
    #[serde(default)]
    pub last_success_at: Option<i64>,
    #[serde(default)]
    pub last_error: Option<String>,
    #[serde(default)]
    pub last_sync_summary: Option<SyncSummary>,
    #[serde(default)]
    pub unread_summary: bool,
}
