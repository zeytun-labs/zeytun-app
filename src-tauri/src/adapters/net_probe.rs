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
    #[cfg(target_os = "windows")]
    {
        let mut interface_ip = String::new();
        if let Ok(out) = std::process::Command::new("route").args(["print", "0.0.0.0"]).output() {
            let stdout = String::from_utf8_lossy(&out.stdout);
            for line in stdout.lines() {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 5 && parts[0] == "0.0.0.0" && parts[1] == "0.0.0.0" {
                    if parts[2] != "On-link" && parts[2] != "127.0.0.1" {
                        interface_ip = parts[3].to_string();
                        break;
                    }
                }
            }
        }

        Ok(LocalNetworkInfo {
            id: interface_ip.clone(),
            name: "Ethernet".to_string(),
            network_type: "ethernet".to_string(),
            local_ip: interface_ip,
        })
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        Err(crate::error::CommandError::Internal(
            "Not supported on this OS".into(),
        ))
    }
}

/// Parse the round-trip time (ms) out of a `ping` command's stdout.
#[cfg(any(target_os = "macos", target_os = "windows"))]
fn parse_ping_time(output: &str) -> u64 {
    for line in output.lines() {
        if let Some(pos) = line.find("time=") {
            let after = &line[pos + 5..];
            let num: String = after
                .chars()
                .take_while(|c| c.is_ascii_digit() || *c == '.')
                .collect();
            if let Ok(v) = num.parse::<f64>() {
                return v.round() as u64;
            }
        } else if line.contains("time<1ms") || line.contains("time<1 ms") {
            return 1;
        }
    }
    0
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

    #[cfg(target_os = "windows")]
    {
        let mut gw = String::new();
        if let Ok(out) = std::process::Command::new("route").args(["print", "0.0.0.0"]).output() {
            let stdout = String::from_utf8_lossy(&out.stdout);
            for line in stdout.lines() {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 5 && parts[0] == "0.0.0.0" && parts[1] == "0.0.0.0" {
                    if parts[2] != "On-link" && parts[2] != "127.0.0.1" {
                        gw = parts[2].to_string();
                        break;
                    }
                }
            }
        }

        if !gw.is_empty() {
            if let Ok(po) = std::process::Command::new("ping").args(["-n", "1", "-w", "1000", &gw]).output() {
                let ping_out = String::from_utf8_lossy(&po.stdout);
                router_ms = parse_ping_time(&ping_out);
            }
        }

        let mut dns_server = "8.8.8.8".to_string();
        if let Ok(out) = std::process::Command::new("netsh").args(["interface", "ip", "show", "dns"]).output() {
            let stdout = String::from_utf8_lossy(&out.stdout);
            for line in stdout.lines() {
                if let Some((_, ip_part)) = line.split_once(':') {
                    let ip = ip_part.trim();
                    if !ip.is_empty() && ip != "None" && ip.contains('.') {
                        dns_server = ip.to_string();
                        break;
                    }
                }
            }
        }

        if let Ok(po) = std::process::Command::new("ping").args(["-n", "1", "-w", "1000", &dns_server]).output() {
            let ping_out = String::from_utf8_lossy(&po.stdout);
            dns_ms = parse_ping_time(&ping_out);
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
    #[cfg(target_os = "windows")]
    {
        Ok("Ethernet".to_string())
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        Ok("Unknown".to_string())
    }
}
