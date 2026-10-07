// src-tauri/src/adapters/system_proxy.rs

use std::process::Command;

pub struct SystemProxyManager;

impl SystemProxyManager {
    /// Enable system proxy across all active/enabled network services.
    /// Attempts all services; succeeds if at least one service was successfully configured.
    pub fn enable(&self, host: &str, port: u16) -> Result<(), String> {
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

#[cfg(test)]
mod tests {
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
}
