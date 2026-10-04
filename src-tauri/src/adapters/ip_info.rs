// src-tauri/src/adapters/ip_info.rs

use std::path::Path;

#[derive(serde::Serialize)]
pub struct IpInfo {
    pub ip: String,
    pub emoji: String,
}

/// Convert an ISO 3166-1 alpha-2 country code into a flag emoji.
fn iso_to_emoji(iso: &str) -> String {
    iso.to_uppercase()
        .chars()
        .map(|c| std::char::from_u32(c as u32 + 127397).unwrap_or(c))
        .collect()
}

/// Fetch the current external IP via the ipify API (blocking ureq call).
///
/// If `proxy_port` is provided, the request is tunneled through the local HTTP
/// proxy (for accurate IP resolution when Global proxy is active).
///
/// Intended to run inside `spawn_blocking` so the synchronous network fetch
/// stays off the async runtime.
pub fn fetch_external_ip(proxy_port: Option<u16>) -> Result<String, crate::error::CommandError> {
    let mut builder = ureq::AgentBuilder::new().timeout(std::time::Duration::from_secs(
        crate::core::constants::IPIFY_API_TIMEOUT_SECS,
    ));

    if let Some(port) = proxy_port {
        let proxy_url = format!("http://127.0.0.1:{port}");
        // Surface a bad proxy instead of silently going direct — a request that
        // quietly bypasses the proxy reports the *real* IP, which is exactly the
        // wrong answer for this call.
        let proxy = ureq::Proxy::new(&proxy_url).map_err(|e| {
            crate::error::CommandError::Internal(format!("invalid proxy URL `{proxy_url}`: {e}"))
        })?;
        builder = builder.proxy(proxy);
    }

    builder
        .build()
        .get(crate::core::constants::IPIFY_API_URL)
        .call()
        .map_err(|e| crate::error::CommandError::Internal(e.to_string()))?
        .into_string()
        .map_err(|e| crate::error::CommandError::Internal(e.to_string()))
}

/// Resolve an IP string to an [`IpInfo`] with a flag emoji, using the GeoLite2
/// country database located at `geolite_db_path`.
pub fn ip_info(ip: String, geolite_db_path: &Path) -> Result<IpInfo, crate::error::CommandError> {
    let ip_addr = ip
        .parse::<std::net::IpAddr>()
        .map_err(|e: std::net::AddrParseError| e.to_string())?;

    let reader = maxminddb::Reader::open_readfile(geolite_db_path)
        .map_err(|e| format!("Failed to open DB at {:?}: {}", geolite_db_path, e))?;

    let mut emoji = String::from("🌐"); // fallback
    let lookup_result = reader
        .lookup(ip_addr)
        .map_err(|e| format!("Lookup error: {}", e))?;
    if let Ok(Some(country_info)) = lookup_result.decode::<maxminddb::geoip2::Country>() {
        let country = country_info.country;
        if let Some(iso) = country.iso_code {
            emoji = iso_to_emoji(iso);
        }
    }

    Ok(IpInfo { ip, emoji })
}
