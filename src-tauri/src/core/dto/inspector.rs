use serde::Serialize;

/// Lifecycle of a single connection as surfaced in the Inspector window.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ConnStatus {
    Active,
    Completed,
    Error,
}

/// Internal per-connection record held in `InspectorState`. Owns its strings;
/// cloned once on connection open, then mutated/moved (no per-tick allocation).
#[derive(Debug, Clone)]
pub struct ConnRecord {
    pub id: String,
    pub status: ConnStatus,
    pub policy: String, // raw outbound tag; UI resolves to display name
    pub is_proxy: bool,
    pub process_name: String,
    pub process_path: String,
    pub host: String,    // domain if known, else destination
    pub address: String, // destination address:port
    pub network: String, // tcp / udp
    pub protocol: String,
    pub rule: String,
    pub up_bytes: i64,
    pub down_bytes: i64,
    pub created_at: i64,       // epoch seconds
    pub ended_at: Option<i64>, // epoch seconds, set on close
}

/// Serialized connection row for the frontend (camelCase) with a computed
/// `durationMs`.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnDto {
    pub id: String,
    pub status: ConnStatus,
    pub policy: String,
    pub is_proxy: bool,
    pub process_name: String,
    pub process_path: String,
    pub host: String,
    pub address: String,
    pub network: String,
    pub protocol: String,
    pub rule: String,
    pub up_bytes: i64,
    pub down_bytes: i64,
    pub created_at: i64,
    pub ended_at: Option<i64>,
    pub duration_ms: i64,
}

impl ConnDto {
    /// Build a DTO from a record, computing duration against `now` for active
    /// rows (those without an end timestamp).
    pub fn from_record(r: &ConnRecord, now: i64) -> Self {
        let end = r.ended_at.unwrap_or(now);
        let duration_ms = (end - r.created_at).max(0) * 1000;
        Self {
            id: r.id.clone(),
            status: r.status,
            policy: r.policy.clone(),
            is_proxy: r.is_proxy,
            process_name: r.process_name.clone(),
            process_path: r.process_path.clone(),
            host: r.host.clone(),
            address: r.address.clone(),
            network: r.network.clone(),
            protocol: r.protocol.clone(),
            rule: r.rule.clone(),
            up_bytes: r.up_bytes,
            down_bytes: r.down_bytes,
            created_at: r.created_at,
            ended_at: r.ended_at,
            duration_ms,
        }
    }
}

/// Full Inspector snapshot returned by `core_inspector_snapshot`.
#[derive(Debug, Clone, Serialize)]
pub struct InspectorSnapshot {
    pub active: Vec<ConnDto>,
    pub recent: Vec<ConnDto>,
}
