//! Which route the app's *own* HTTP traffic takes.
//!
//! The app can only dial `direct` or its own mixed inbound; policy tags are
//! resolvable by the core alone. [`NetworkPolicy::Policy`] therefore degrades to
//! [`NetworkPolicy::LocalProxy`] for app-originated requests, and is honoured
//! verbatim only for rule-sets, where the core does the dialing itself via
//! `http_client.detour`.
//!
//! An absent stored value means [`NetworkPolicy::Direct`] — see
//! `migrations/0018_network_policy.sql`.

use serde::{Deserialize, Serialize};

/// Traffic classes whose route the user can choose. Connectivity diagnostics are
/// deliberately absent: the internet probe exists to measure the *unproxied*
/// path, and the proxy probe already goes through the proxy by definition.
pub const TRAFFIC_APP_UPDATE: &str = "app_update";
pub const TRAFFIC_GEOIP: &str = "geoip";
pub const TRAFFIC_SUBSCRIPTION: &str = "subscription";

/// All valid traffic keys, for validating input from the frontend.
pub const TRAFFIC_KINDS: [&str; 3] = [TRAFFIC_APP_UPDATE, TRAFFIC_GEOIP, TRAFFIC_SUBSCRIPTION];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "tag", rename_all = "snake_case")]
#[derive(Default)]
pub enum NetworkPolicy {
    #[default]
    Direct,
    LocalProxy,
    Policy(String),
}

impl NetworkPolicy {
    /// `(proxy_url, degraded)`. `degraded` is true when the caller asked for a
    /// named policy that only the core can honour, so callers can log or surface
    /// the difference once instead of pretending the request went where it was
    /// told to go.
    pub fn resolve(&self, mixed_port: u16) -> (Option<String>, bool) {
        match self {
            Self::Direct => (None, false),
            Self::LocalProxy => (Some(format!("http://127.0.0.1:{mixed_port}")), false),
            Self::Policy(_) => (Some(format!("http://127.0.0.1:{mixed_port}")), true),
        }
    }

    /// Detour tag for the core's `http_client`. `None` → plain dialer.
    ///
    /// ponytail: mirrors the existing rule-set default of "direct"; a real
    /// per-traffic core detour would need the core to dial on the app's behalf.
    pub fn core_detour(&self) -> Option<&str> {
        match self {
            Self::Direct | Self::LocalProxy => None,
            Self::Policy(tag) => Some(tag.as_str()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn direct_resolves_to_no_proxy() {
        assert_eq!(NetworkPolicy::Direct.resolve(6060), (None, false));
    }

    #[test]
    fn local_proxy_resolves_to_mixed_port() {
        let (url, degraded) = NetworkPolicy::LocalProxy.resolve(6060);
        assert_eq!(url.as_deref(), Some("http://127.0.0.1:6060"));
        assert!(!degraded);
    }

    #[test]
    fn named_policy_degrades_to_local_proxy() {
        let (url, degraded) = NetworkPolicy::Policy("us-auto".into()).resolve(7070);
        assert_eq!(url.as_deref(), Some("http://127.0.0.1:7070"));
        assert!(
            degraded,
            "app-side traffic cannot dial a policy tag directly"
        );
    }

    #[test]
    fn core_detour_only_for_named_policy() {
        assert_eq!(NetworkPolicy::Direct.core_detour(), None);
        assert_eq!(NetworkPolicy::LocalProxy.core_detour(), None);
        assert_eq!(
            NetworkPolicy::Policy("us-auto".into()).core_detour(),
            Some("us-auto")
        );
    }

    #[test]
    fn default_is_direct() {
        assert_eq!(NetworkPolicy::default(), NetworkPolicy::Direct);
    }

    /// The wire shape the frontend sends. Locked down because a silent rename
    /// would make every stored row unreadable.
    #[test]
    fn serde_shape_is_stable() {
        let direct = serde_json::to_string(&NetworkPolicy::Direct).unwrap();
        assert_eq!(direct, r#"{"kind":"direct"}"#);

        let local = serde_json::to_string(&NetworkPolicy::LocalProxy).unwrap();
        assert_eq!(local, r#"{"kind":"local_proxy"}"#);

        let policy = serde_json::to_string(&NetworkPolicy::Policy("us-auto".into())).unwrap();
        assert_eq!(policy, r#"{"kind":"policy","tag":"us-auto"}"#);

        let back: NetworkPolicy = serde_json::from_str(r#"{"kind":"policy","tag":"x"}"#).unwrap();
        assert_eq!(back, NetworkPolicy::Policy("x".into()));
    }
}
