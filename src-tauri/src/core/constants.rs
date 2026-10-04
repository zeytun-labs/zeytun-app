// API URLs
pub const IPIFY_API_URL: &str = "https://api.ipify.org";
pub const DEFAULT_STUN_SERVER: &str = "stun.voipgate.com:3478";

// External Resources
// GeoIP country database: DB-IP Country Lite (CC BY 4.0). Attribution is shown in
// Settings → Network and is a licence obligation, not decoration.
pub const GEOIP_COUNTRY_PATH: &str = "resources/geoip.mmdb";
/// Filename of the downloaded copy in the app data directory.
pub const GEOIP_DB_FILENAME: &str = "geoip.mmdb";

// Ports & Addresses
pub const CLASH_API_PORT: u16 = 9090;
pub const GRPC_API_PORT: u16 = 9999;
pub const GRPC_API_ENDPOINT: &str = "http://127.0.0.1:9999";
pub const LOCALHOST: &str = "127.0.0.1";

// Wait & Retry Timeouts
pub const CLASH_API_SELECT_RETRY_COUNT: usize = 10;
pub const CLASH_API_SELECT_RETRY_DELAY_MS: u64 = 50;
pub const DAEMON_RECONNECT_INTERVAL_SECS: u64 = 2;
pub const IPIFY_API_TIMEOUT_SECS: u64 = 5;

/// Current UNIX time in whole milliseconds.
///
/// One shared implementation for DB timestamps and sync metadata; the pre-epoch
/// fallback yields 0 so a clock set before 1970 cannot panic a save path.
pub fn unix_millis() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or_default()
}
