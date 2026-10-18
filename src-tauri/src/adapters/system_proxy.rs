// src-tauri/src/adapters/system_proxy.rs

pub struct SystemProxyManager;

/// Normalize proxy target host. `0.0.0.0`, `::`, or empty string are unspecified
/// bind addresses (INADDR_ANY) and illegal as connect destinations on Windows
/// (WSAEADDRNOTAVAIL 10049). Local OS system proxy must always target loopback.
fn normalize_proxy_host(host: &str) -> &str {
    match host {
        "0.0.0.0" | "::" | "" => "127.0.0.1",
        other => other,
    }
}

#[cfg(target_os = "macos")]
impl SystemProxyManager {
    /// Enable system proxy across all active/enabled network services.
    /// Attempts all services; succeeds if at least one service was successfully configured.
    pub fn enable(&self, host: &str, port: u16) -> Result<(), String> {
        let host = normalize_proxy_host(host);
        let services = self.network_services();
        let port_str = port.to_string();
        let mut first_error = None;
        let mut any_success = false;

        for service in &services {
            let mut service_ok = true;
            for args in [
                &["-setwebproxy", service.as_str(), host, port_str.as_str()][..],
                &[
                    "-setsecurewebproxy",
                    service.as_str(),
                    host,
                    port_str.as_str(),
                ][..],
                &[
                    "-setsocksfirewallproxy",
                    service.as_str(),
                    host,
                    port_str.as_str(),
                ][..],
                &["-setwebproxystate", service.as_str(), "on"][..],
                &["-setsecurewebproxystate", service.as_str(), "on"][..],
                &["-setsocksfirewallproxystate", service.as_str(), "on"][..],
            ] {
                if let Err(err) = self.run_networksetup(args) {
                    first_error.get_or_insert(err);
                    service_ok = false;
                }
            }
            if service_ok {
                any_success = true;
            }
        }

        if any_success || services.is_empty() {
            Ok(())
        } else {
            first_error.map_or(Ok(()), Err)
        }
    }

    /// Disable system proxy across all network services.
    pub fn disable(&self) -> Result<(), String> {
        let mut first_error = None;
        for service in self.network_services() {
            for setting in [
                "-setwebproxystate",
                "-setsecurewebproxystate",
                "-setsocksfirewallproxystate",
            ] {
                if let Err(error) = self.run_networksetup(&[setting, &service, "off"]) {
                    first_error.get_or_insert(error);
                }
            }
        }
        first_error.map_or(Ok(()), Err)
    }

    /// Discover all active, non-disabled network services via `networksetup`.
    /// Filters out disabled services (prefixed with `*`) and header lines.
    /// Falls back to `["Wi-Fi"]` if `networksetup` fails.
    fn network_services(&self) -> Vec<String> {
        use std::process::Command;

        if let Ok(output) = Command::new("networksetup")
            .arg("-listallnetworkservices")
            .output()
        {
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                let services: Vec<String> = stdout
                    .lines()
                    .map(|line| line.trim())
                    .filter(|line| !line.is_empty())
                    .filter(|line| !line.starts_with('*'))
                    .filter(|line| !line.starts_with("An asterisk"))
                    .map(|line| line.to_string())
                    .collect();
                if !services.is_empty() {
                    return services;
                }
            }
        }
        vec!["Wi-Fi".to_string()]
    }

    fn run_networksetup(&self, args: &[&str]) -> Result<(), String> {
        use std::process::Command;

        let output = Command::new("networksetup")
            .args(args)
            .output()
            .map_err(|e| e.to_string())?;

        if !output.status.success() {
            return Err(String::from_utf8_lossy(&output.stderr).to_string());
        }

        Ok(())
    }
}

#[cfg(target_os = "windows")]
impl SystemProxyManager {
    const INTERNET_SETTINGS: &'static str =
        r"Software\Microsoft\Windows\CurrentVersion\Internet Settings";

    /// Enable system proxy in Windows registry and notify WinINet.
    pub fn enable(&self, host: &str, port: u16) -> Result<(), String> {
        use winreg::enums::{HKEY_CURRENT_USER, KEY_SET_VALUE};
        use winreg::RegKey;

        let effective_host = normalize_proxy_host(host);
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        let (key, _) = hkcu
            .create_subkey_with_flags(Self::INTERNET_SETTINGS, KEY_SET_VALUE)
            .map_err(|e| format!("Failed to open Internet Settings registry key: {e}"))?;

        let proxy_server = format!("{}:{}", effective_host, port);
        key.set_value("ProxyServer", &proxy_server)
            .map_err(|e| format!("Failed to set ProxyServer: {e}"))?;

        key.set_value("ProxyEnable", &1u32)
            .map_err(|e| format!("Failed to set ProxyEnable: {e}"))?;

        let proxy_override = "<local>;localhost;127.*";
        let _ = key.set_value("ProxyOverride", &proxy_override);

        self.refresh_wininet();
        Ok(())
    }

    /// Disable system proxy in Windows registry and notify WinINet.
    pub fn disable(&self) -> Result<(), String> {
        use winreg::enums::{HKEY_CURRENT_USER, KEY_SET_VALUE};
        use winreg::RegKey;

        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        let (key, _) = hkcu
            .create_subkey_with_flags(Self::INTERNET_SETTINGS, KEY_SET_VALUE)
            .map_err(|e| format!("Failed to open Internet Settings registry key: {e}"))?;

        key.set_value("ProxyEnable", &0u32)
            .map_err(|e| format!("Failed to set ProxyEnable: {e}"))?;

        self.refresh_wininet();
        Ok(())
    }

    fn refresh_wininet(&self) {
        use windows::Win32::Networking::WinInet::{
            InternetSetOptionW, INTERNET_OPTION_REFRESH, INTERNET_OPTION_SETTINGS_CHANGED,
        };

        unsafe {
            let _ = InternetSetOptionW(None, INTERNET_OPTION_SETTINGS_CHANGED, None, 0);
            let _ = InternetSetOptionW(None, INTERNET_OPTION_REFRESH, None, 0);
        }
    }
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
impl SystemProxyManager {
    pub fn enable(&self, _host: &str, _port: u16) -> Result<(), String> {
        Ok(())
    }

    pub fn disable(&self) -> Result<(), String> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_proxy_host() {
        assert_eq!(normalize_proxy_host("0.0.0.0"), "127.0.0.1");
        assert_eq!(normalize_proxy_host("::"), "127.0.0.1");
        assert_eq!(normalize_proxy_host(""), "127.0.0.1");
        assert_eq!(normalize_proxy_host("127.0.0.1"), "127.0.0.1");
        assert_eq!(normalize_proxy_host("localhost"), "localhost");
        assert_eq!(normalize_proxy_host("192.168.1.100"), "192.168.1.100");
    }

    #[test]
    fn parses_network_services_output_ignoring_disabled_and_headers() {
        let sample = "An asterisk (*) denotes that a network service is disabled.\nAX88179A\n*Thunderbolt Bridge\nWi-Fi\n*Surge\n";
        let services: Vec<String> = sample
            .lines()
            .map(|line| line.trim())
            .filter(|line| !line.is_empty())
            .filter(|line| !line.starts_with('*'))
            .filter(|line| !line.starts_with("An asterisk"))
            .map(|line| line.to_string())
            .collect();

        assert_eq!(services, vec!["AX88179A".to_string(), "Wi-Fi".to_string()]);
    }

    #[test]
    #[cfg(target_os = "windows")]
    fn windows_proxy_roundtrip() {
        use winreg::enums::HKEY_CURRENT_USER;
        use winreg::RegKey;

        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        let key = hkcu
            .open_subkey(super::SystemProxyManager::INTERNET_SETTINGS)
            .unwrap();
        let orig_enable: Option<u32> = key.get_value("ProxyEnable").ok();
        let orig_server: Option<String> = key.get_value("ProxyServer").ok();

        let mgr = super::SystemProxyManager;
        mgr.enable("0.0.0.0", 39999).unwrap();

        let key = hkcu
            .open_subkey(super::SystemProxyManager::INTERNET_SETTINGS)
            .unwrap();
        let new_enable: u32 = key.get_value("ProxyEnable").unwrap();
        let new_server: String = key.get_value("ProxyServer").unwrap();
        assert_eq!(new_enable, 1);
        assert_eq!(new_server, "127.0.0.1:39999");

        // Restore original user settings
        if let Some(orig_enable) = orig_enable {
            if orig_enable == 1 {
                if let Some(orig_server) = orig_server {
                    let parts: Vec<&str> = orig_server.split(':').collect();
                    let port = parts.get(1).and_then(|p| p.parse().ok()).unwrap_or(8080);
                    let host = parts.first().copied().unwrap_or("127.0.0.1");
                    let _ = mgr.enable(host, port);
                }
            } else {
                let _ = mgr.disable();
            }
        }
    }
}
