// src-tauri/src/adapters/system_proxy.rs

use std::process::Command;

pub struct SystemProxyManager;

impl SystemProxyManager {
    pub fn enable(&self, host: &str, port: u16) -> Result<(), String> {
        let services = self.network_services();

        for service in services {
            self.run_networksetup(&["-setwebproxy", &service, host, &port.to_string()])?;
            self.run_networksetup(&["-setsecurewebproxy", &service, host, &port.to_string()])?;
            self.run_networksetup(&["-setsocksfirewallproxy", &service, host, &port.to_string()])?;

            self.run_networksetup(&["-setwebproxystate", &service, "on"])?;
            self.run_networksetup(&["-setsecurewebproxystate", &service, "on"])?;
            self.run_networksetup(&["-setsocksfirewallproxystate", &service, "on"])?;
        }

        Ok(())
    }

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

    fn network_services(&self) -> Vec<String> {
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
