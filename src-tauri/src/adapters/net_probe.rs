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

        // 4. Determine network_type and formatted display name
        let lower_name = name.to_lowercase();
        let is_wifi = lower_name.contains("wi-fi") || lower_name.contains("wifi");
        let network_type = if is_wifi {
            "wifi"
        } else if lower_name.contains("iphone") || lower_name.contains("cellular") {
            "cellular"
        } else if lower_name.contains("thunderbolt") {
            "thunderbolt"
        } else {
            "ethernet"
        }
        .to_string();

        if is_wifi {
            if let Ok(out) = std::process::Command::new("networksetup")
                .args(["-getairportnetwork", &interface])
                .output()
            {
                let stdout = String::from_utf8_lossy(&out.stdout);
                if let Some((_, ssid)) = stdout.trim().split_once(':') {
                    let ssid = ssid.trim();
                    if !ssid.is_empty() && !ssid.contains("not associated") {
                        name = format!("Wi-Fi ({ssid})");
                    } else {
                        name = "Wi-Fi".to_string();
                    }
                } else {
                    name = "Wi-Fi".to_string();
                }
            } else {
                name = "Wi-Fi".to_string();
            }
        } else if network_type == "ethernet" {
            name = "Ethernet".to_string();
        }

        Ok(LocalNetworkInfo {
            id: interface,
            name,
            network_type,
            local_ip,
        })
    }
    #[cfg(target_os = "windows")]
    {
        let adapter_info = windows_net::get_adapter_details();
        let profile_name = windows_net::get_network_profile_name();

        let network_type = adapter_info.network_type;
        let name = match network_type.as_str() {
            "wifi" => match profile_name {
                Some(ssid) if !ssid.is_empty() && ssid != "UNKNOWN" => {
                    format!("Wi-Fi ({ssid})")
                }
                _ => "Wi-Fi".to_string(),
            },
            "ethernet" => "Ethernet".to_string(),
            _ => {
                if !adapter_info.interface_name.is_empty() {
                    adapter_info.interface_name.clone()
                } else {
                    "UNKNOWN".to_string()
                }
            }
        };

        let id = if !adapter_info.interface_name.is_empty() {
            adapter_info.interface_name
        } else if !adapter_info.local_ip.is_empty() {
            adapter_info.local_ip.clone()
        } else {
            "UNKNOWN".to_string()
        };

        Ok(LocalNetworkInfo {
            id,
            name,
            network_type,
            local_ip: adapter_info.local_ip,
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
        if let Ok(out) = std::process::Command::new("route")
            .args(["print", "0.0.0.0"])
            .output()
        {
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
            if let Ok(po) = std::process::Command::new("ping")
                .args(["-n", "1", "-w", "1000", &gw])
                .output()
            {
                let ping_out = String::from_utf8_lossy(&po.stdout);
                router_ms = parse_ping_time(&ping_out);
            }
        }

        let mut dns_server = "8.8.8.8".to_string();
        if let Ok(out) = std::process::Command::new("netsh")
            .args(["interface", "ip", "show", "dns"])
            .output()
        {
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

        if let Ok(po) = std::process::Command::new("ping")
            .args(["-n", "1", "-w", "1000", &dns_server])
            .output()
        {
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
                        let lower = name.to_lowercase();
                        if lower.contains("wi-fi") || lower.contains("wifi") {
                            if let Ok(out) = std::process::Command::new("networksetup")
                                .args(["-getairportnetwork", interface])
                                .output()
                            {
                                let stdout = String::from_utf8_lossy(&out.stdout);
                                if let Some((_, ssid)) = stdout.trim().split_once(':') {
                                    let ssid = ssid.trim();
                                    if !ssid.is_empty() && !ssid.contains("not associated") {
                                        return Ok(format!("Wi-Fi ({ssid})"));
                                    }
                                }
                            }
                            return Ok("Wi-Fi".to_string());
                        } else if lower.contains("ethernet") {
                            return Ok("Ethernet".to_string());
                        } else if !name.is_empty() {
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
        let adapter_info = windows_net::get_adapter_details();
        let profile_name = windows_net::get_network_profile_name();
        match adapter_info.network_type.as_str() {
            "wifi" => {
                if let Some(ssid) = profile_name {
                    if !ssid.is_empty() && ssid != "UNKNOWN" {
                        return Ok(format!("Wi-Fi ({ssid})"));
                    }
                }
                Ok("Wi-Fi".to_string())
            }
            "ethernet" => Ok("Ethernet".to_string()),
            _ => {
                if !adapter_info.interface_name.is_empty() {
                    Ok(adapter_info.interface_name)
                } else {
                    Ok("UNKNOWN".to_string())
                }
            }
        }
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        Ok("Unknown".to_string())
    }
}

#[cfg(target_os = "windows")]
mod windows_net {
    use std::net::Ipv4Addr;
    use windows::Win32::NetworkManagement::IpHelper::{
        GetAdaptersAddresses, GAA_FLAG_INCLUDE_ALL_INTERFACES, GAA_FLAG_INCLUDE_GATEWAYS,
        IP_ADAPTER_ADDRESSES_LH,
    };
    use windows::Win32::Networking::NetworkListManager::{
        INetworkListManager, NetworkListManager, NLM_ENUM_NETWORK_CONNECTED,
    };
    use windows::Win32::Networking::WinSock::{AF_UNSPEC, SOCKADDR_IN};
    use windows::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_ALL, COINIT_MULTITHREADED,
    };

    const IF_TYPE_IEEE80211: u32 = 71;
    const IF_TYPE_ETHERNET_CSMACD: u32 = 6;
    const IF_OPER_STATUS_UP: i32 = 1;

    struct ComGuard {
        need_uninit: bool,
    }

    impl ComGuard {
        fn init() -> Self {
            let hr = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
            Self {
                need_uninit: hr.is_ok(),
            }
        }
    }

    impl Drop for ComGuard {
        fn drop(&mut self) {
            if self.need_uninit {
                unsafe { CoUninitialize() };
            }
        }
    }

    /// Retrieve the active connected network profile name via Network List Manager (NLM).
    /// Standard user permission, no elevation required.
    pub fn get_network_profile_name() -> Option<String> {
        let _com = ComGuard::init();
        unsafe {
            let nlm: INetworkListManager =
                CoCreateInstance(&NetworkListManager, None, CLSCTX_ALL).ok()?;
            let networks = nlm.GetNetworks(NLM_ENUM_NETWORK_CONNECTED).ok()?;
            let mut item = [None; 1];
            let mut fetched = 0u32;
            let mut first_connected = None;

            while networks.Next(&mut item, Some(&mut fetched)).is_ok() && fetched > 0 {
                if let Some(network) = item[0].take() {
                    let name = network
                        .GetName()
                        .map(|b| b.to_string())
                        .unwrap_or_default()
                        .trim()
                        .to_string();
                    if name.is_empty() {
                        continue;
                    }
                    let is_internet = network
                        .IsConnectedToInternet()
                        .map(|b| b.as_bool())
                        .unwrap_or(false);
                    if is_internet {
                        return Some(name);
                    }
                    if first_connected.is_none() {
                        first_connected = Some(name);
                    }
                }
            }
            first_connected
        }
    }

    pub struct WindowsAdapterInfo {
        pub interface_name: String,
        pub local_ip: String,
        pub network_type: String,
    }

    /// Query network adapters via IP Helper API `GetAdaptersAddresses`.
    /// Identifies the default gateway adapter, local IPv4, and physical medium.
    pub fn get_adapter_details() -> WindowsAdapterInfo {
        let mut buf_len = 16384u32;
        let mut buf = vec![0u8; buf_len as usize];
        let flags = GAA_FLAG_INCLUDE_GATEWAYS | GAA_FLAG_INCLUDE_ALL_INTERFACES;

        unsafe {
            let mut ret = GetAdaptersAddresses(
                AF_UNSPEC.0 as u32,
                flags,
                None,
                Some(buf.as_mut_ptr() as *mut IP_ADAPTER_ADDRESSES_LH),
                &mut buf_len,
            );
            if ret == 111 {
                // ERROR_BUFFER_OVERFLOW
                buf.resize(buf_len as usize, 0);
                ret = GetAdaptersAddresses(
                    AF_UNSPEC.0 as u32,
                    flags,
                    None,
                    Some(buf.as_mut_ptr() as *mut IP_ADAPTER_ADDRESSES_LH),
                    &mut buf_len,
                );
            }
            if ret != 0 {
                return WindowsAdapterInfo {
                    interface_name: String::new(),
                    local_ip: String::new(),
                    network_type: "unknown".to_string(),
                };
            }

            let mut has_active_wifi = false;
            let mut gw_adapter_name = String::new();
            let mut gw_adapter_ip = String::new();
            let mut gw_if_type = 0u32;

            let mut curr = buf.as_ptr() as *const IP_ADAPTER_ADDRESSES_LH;
            while !curr.is_null() {
                let adapter = &*curr;
                let oper_status = adapter.OperStatus.0;
                let if_type = adapter.IfType;
                let friendly_name = adapter.FriendlyName.to_string().unwrap_or_default();
                let desc = adapter
                    .Description
                    .to_string()
                    .unwrap_or_default()
                    .to_lowercase();
                let is_filter_or_sub = desc.contains("filter")
                    || desc.contains("lightweight")
                    || friendly_name.contains("Filter")
                    || friendly_name.contains("Direct");

                if oper_status == IF_OPER_STATUS_UP && !is_filter_or_sub {
                    if if_type == IF_TYPE_IEEE80211
                        || desc.contains("wi-fi")
                        || desc.contains("wireless")
                        || desc.contains("802.11")
                        || friendly_name.to_lowercase().contains("wi-fi")
                    {
                        has_active_wifi = true;
                    }

                    if !adapter.FirstGatewayAddress.is_null() && gw_adapter_ip.is_empty() {
                        let mut unicast = adapter.FirstUnicastAddress;
                        while !unicast.is_null() {
                            let u = &*unicast;
                            let sockaddr = u.Address.lpSockaddr;
                            if !sockaddr.is_null() && (*sockaddr).sa_family.0 == 2 {
                                // AF_INET = 2
                                let sin = sockaddr as *const SOCKADDR_IN;
                                let ip_bytes = (*sin).sin_addr.S_un.S_addr.to_ne_bytes();
                                let ip = Ipv4Addr::new(
                                    ip_bytes[0],
                                    ip_bytes[1],
                                    ip_bytes[2],
                                    ip_bytes[3],
                                );
                                gw_adapter_ip = ip.to_string();
                                gw_adapter_name = friendly_name.clone();
                                gw_if_type = if_type;
                                break;
                            }
                            unicast = (*unicast).Next;
                        }
                    }
                }
                curr = adapter.Next;
            }

            let network_type = if has_active_wifi {
                "wifi"
            } else if gw_if_type == IF_TYPE_ETHERNET_CSMACD {
                "ethernet"
            } else if !gw_adapter_ip.is_empty() {
                "ethernet"
            } else {
                "unknown"
            }
            .to_string();

            WindowsAdapterInfo {
                interface_name: gw_adapter_name,
                local_ip: gw_adapter_ip,
                network_type,
            }
        }
    }
}

#[cfg(test)]
#[cfg(target_os = "windows")]
mod test_win_net {
    use super::*;

    #[test]
    fn test_windows_local_network_info() {
        let info = local_network_info().expect("local_network_info should succeed");
        println!(
            "Detected Windows network: id='{}', name='{}', type='{}', ip='{}'",
            info.id, info.name, info.network_type, info.local_ip
        );
        assert!(!info.name.is_empty());
        assert!(!info.network_type.is_empty());
        assert_ne!(info.name, "UNKNOWN");
        assert_eq!(info.network_type, "wifi");
        assert!(info.name.starts_with("Wi-Fi ("));
        assert!(info.name.ends_with(')'));
        assert!(!info.local_ip.is_empty());

        let active_name = active_interface().expect("active name");
        assert_eq!(active_name, info.name);
    }
}
