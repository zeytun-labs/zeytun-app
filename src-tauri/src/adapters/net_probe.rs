// src-tauri/src/adapters/net_probe.rs

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalNetworkInfo {
    pub id: String,
    pub name: String,
    pub network_type: String,
    pub local_ip: String,
}

pub fn local_network_info() -> Result<LocalNetworkInfo, crate::error::CommandError> {
    #[cfg(target_os = "macos")]
    {
        // 1. Get default interface (e.g. en0)
        let mut interface = String::new();
        if let Ok(out) = std::process::Command::new("route")
            .args(["-n", "get", "default"])
            .output()
        {
            let stdout = String::from_utf8_lossy(&out.stdout);
            if let Some(iface) = stdout
                .lines()
                .find(|l| l.trim_start().starts_with("interface:"))
                .and_then(|l| l.split_whitespace().nth(1))
            {
                interface = iface.trim().to_string();
            }
        }

        if interface.is_empty() {
            return Err(crate::error::CommandError::Internal(
                "No active interface found".into(),
            ));
        }

        // 2. Get local IP
        let mut local_ip = String::new();
        if let Ok(out) = std::process::Command::new("ipconfig")
            .args(["getifaddr", &interface])
            .output()
        {
            local_ip = String::from_utf8_lossy(&out.stdout).trim().to_string();
        }

        // 3. Get Hardware name from networksetup
        let mut name = interface.clone();
        if let Ok(out) = std::process::Command::new("networksetup")
            .arg("-listallhardwareports")
            .output()
        {
            let stdout = String::from_utf8_lossy(&out.stdout);
            let mut current_port = None;
            for line in stdout.lines() {
                if line.starts_with("Hardware Port: ") {
                    current_port = Some(
                        line.trim_start_matches("Hardware Port: ")
                            .trim()
                            .to_string(),
                    );
                } else if line.starts_with("Device: ") {
                    let dev = line.trim_start_matches("Device: ").trim();
                    if dev == interface {
                        if let Some(port_name) = current_port {
                            name = port_name;
                            break;
                        }
                    }
                }
            }
        }

        // 4. Determine network_type
        let lower_name = name.to_lowercase();
        let network_type = if lower_name.contains("wi-fi") || lower_name.contains("wifi") {
            "wifi"
        } else if lower_name.contains("iphone") || lower_name.contains("cellular") {
            "cellular"
        } else if lower_name.contains("thunderbolt") {
            "thunderbolt"
        } else {
            "ethernet"
        }
        .to_string();

        Ok(LocalNetworkInfo {
            id: interface,
            name,
            network_type,
            local_ip,
        })
    }
    #[cfg(not(target_os = "macos"))]
    {
        Err(crate::error::CommandError::Internal(
            "Not supported on this OS".into(),
        ))
    }
}

/// Parse the round-trip time (ms) out of a `ping` command's stdout.
#[cfg(target_os = "macos")]
fn parse_ping_time(output: &str) -> u64 {
    output
        .lines()
        .find(|l| l.contains("time="))
        .and_then(|l| {
            let after = l.split("time=").nth(1)?;
            let num_str: String = after
                .chars()
                .take_while(|c| c.is_ascii_digit() || *c == '.')
                .collect();
            num_str.parse::<f64>().ok()
        })
        .map(|v| v.round() as u64)
        .unwrap_or(0)
}

/// Measure latency to the default gateway and the configured DNS server.
///
/// Shells out to `route`/`scutil`/`ping` (macOS). Blocking; call inside
/// `spawn_blocking`. Returns `{ "router": <ms>, "dns": <ms> }`.
pub fn local_latencies() -> Result<serde_json::Value, crate::error::CommandError> {
    let mut router_ms = 0u64;
    let mut dns_ms = 0u64;

    #[cfg(target_os = "macos")]
    {
        if let Ok(out) = std::process::Command::new("route")
            .args(["-n", "get", "default"])
            .output()
        {
            let stdout = String::from_utf8_lossy(&out.stdout);
            let gw = stdout
                .lines()
                .find(|l| l.contains("gateway"))
                .and_then(|l| l.split_whitespace().nth(1))
                .unwrap_or("")
                .trim();
            if !gw.is_empty() {
                if let Ok(po) = std::process::Command::new("ping")
                    .args(["-c", "1", "-t", "1", gw])
                    .output()
                {
                    let ping_out = String::from_utf8_lossy(&po.stdout);
                    router_ms = parse_ping_time(&ping_out);
                }
            }
        }

        if let Ok(out) = std::process::Command::new("scutil").arg("--dns").output() {
            let stdout = String::from_utf8_lossy(&out.stdout);
            let dns_server = stdout
                .lines()
                .find(|l| l.trim_start().starts_with("nameserver"))
                .and_then(|l| l.split_once(':').map(|(_, ip)| ip.trim().to_string()))
                .unwrap_or_else(|| "8.8.8.8".to_string());
            if !dns_server.is_empty() {
                if let Ok(po) = std::process::Command::new("ping")
                    .args(["-c", "1", "-t", "1", &dns_server])
                    .output()
                {
                    let ping_out = String::from_utf8_lossy(&po.stdout);
                    dns_ms = parse_ping_time(&ping_out);
                }
            }
        }
    }

    Ok(serde_json::json!({
        "router": router_ms,
        "dns": dns_ms
    }))
}

/// Determine the human-friendly name of the active network interface.
///
/// Shells out to `route`/`networksetup` (macOS). Blocking; call inside
/// `spawn_blocking`. Returns `"Unknown"` when it cannot be determined.
pub fn active_interface() -> Result<String, crate::error::CommandError> {
    #[cfg(target_os = "macos")]
    {
        let output = std::process::Command::new("route")
            .args(["get", "default"])
            .output()
            .map_err(|e| crate::error::CommandError::Internal(e.to_string()))?;

        if !output.status.success() {
            return Ok("Unknown".to_string());
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let interface = stdout
            .lines()
            .find(|l| l.trim_start().starts_with("interface"))
            .and_then(|l| l.split_whitespace().nth(1))
            .unwrap_or("")
            .trim();
        if interface.is_empty() {
            return Ok("Unknown".to_string());
        }

        if let Ok(ns_output) = std::process::Command::new("networksetup")
            .arg("-listnetworkserviceorder")
            .output()
        {
            let ns_stdout = String::from_utf8_lossy(&ns_output.stdout);
            for line in ns_stdout.lines() {
                let trimmed = line.trim();
                if trimmed.contains(&format!("Device: {}", interface)) {
                    if let Some(name) = trimmed.split(')').nth(1) {
                        let name = name.trim();
                        if !name.is_empty() {
                            return Ok(name.to_string());
                        }
                    }
                }
            }
        }

        Ok(interface.to_string())
    }
    #[cfg(not(target_os = "macos"))]
    {
        Ok("Unknown".to_string())
    }
}
