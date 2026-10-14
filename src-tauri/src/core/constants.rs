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
#[cfg(all(debug_assertions, not(feature = "updater-lab")))]
pub const CLASH_API_PORT: u16 = 19090;
#[cfg(all(not(debug_assertions), not(feature = "updater-lab")))]
pub const CLASH_API_PORT: u16 = 9090;

#[cfg(all(debug_assertions, not(feature = "updater-lab")))]
pub const GRPC_API_PORT: u16 = 19999;
#[cfg(all(not(debug_assertions), not(feature = "updater-lab")))]
pub const GRPC_API_PORT: u16 = 9999;

#[cfg(all(debug_assertions, not(feature = "updater-lab")))]
pub const GRPC_API_ENDPOINT: &str = "http://127.0.0.1:19999";
#[cfg(all(not(debug_assertions), not(feature = "updater-lab")))]
pub const GRPC_API_ENDPOINT: &str = "http://127.0.0.1:9999";

#[cfg(all(debug_assertions, not(feature = "updater-lab")))]
pub const DEFAULT_MIXED_PORT: u16 = 16060;
#[cfg(all(not(debug_assertions), not(feature = "updater-lab")))]
pub const DEFAULT_MIXED_PORT: u16 = 6060;

#[cfg(all(debug_assertions, not(feature = "updater-lab")))]
pub const DEFAULT_SOCKS_PORT: u16 = 16061;
#[cfg(all(not(debug_assertions), not(feature = "updater-lab")))]
pub const DEFAULT_SOCKS_PORT: u16 = 6061;

#[cfg(all(debug_assertions, not(feature = "updater-lab")))]
pub const DEFAULT_HTTP_PORT: u16 = 16062;
#[cfg(all(not(debug_assertions), not(feature = "updater-lab")))]
pub const DEFAULT_HTTP_PORT: u16 = 6062;

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

// Lab release bundles must not connect to or control a production/dev core.
#[cfg(feature = "updater-lab")]
pub const CLASH_API_PORT: u16 = 29090;
#[cfg(feature = "updater-lab")]
pub const GRPC_API_PORT: u16 = 29999;
#[cfg(feature = "updater-lab")]
pub const GRPC_API_ENDPOINT: &str = "http://127.0.0.1:29999";
#[cfg(feature = "updater-lab")]
pub const DEFAULT_MIXED_PORT: u16 = 26060;
#[cfg(feature = "updater-lab")]
pub const DEFAULT_SOCKS_PORT: u16 = 26061;
#[cfg(feature = "updater-lab")]
pub const DEFAULT_HTTP_PORT: u16 = 26062;
