use serde::{Deserialize, Serialize};

/// A single aggregated traffic delta to upsert. The daemon buffers these by the
/// (ts_minute, domain, policy, process, is_proxy) key before flushing in a batch.
#[derive(Debug, Clone)]
pub struct TrafficDelta {
    pub ts_minute: i64,
    pub domain: String,
    pub policy: String,
    pub process: String,
    pub is_proxy: bool,
    pub up_bytes: i64,
    pub down_bytes: i64,
}

/// One point on the 18h time-series (per-minute bucket, summed across dims).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrafficPoint {
    pub ts: i64, // epoch seconds, minute-truncated
    pub up: i64,
    pub down: i64,
}

/// A Top-X consumer row (domain / policy / process).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrafficTopEntry {
    pub name: String,
    pub up: i64,
    pub down: i64,
}

/// Full payload for `core_get_traffic_analytics`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrafficAnalytics {
    pub series: Vec<TrafficPoint>,
    pub top_domains: Vec<TrafficTopEntry>,
    pub top_policies: Vec<TrafficTopEntry>,
    pub top_processes: Vec<TrafficTopEntry>,
}

/// Direct-vs-proxy totals over a period, for the summary card.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrafficSummary {
    pub direct_bytes: i64,
    pub proxy_bytes: i64,
}
