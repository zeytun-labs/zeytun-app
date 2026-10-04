use std::{path::Path, process::Command, sync::Arc};

use crate::{
    adapters::{
        clash_api::ClashApiClient, config_store::ConfigStore, system_proxy::SystemProxyManager,
        zeytun_core_process::ZeytunCoreProcessManager,
    },
    core::{
        compiler::{
            profile_converter::ProfileConverter, runtime_profile_compiler::RuntimeProfileCompiler,
        },
        dto::{
            BinaryCheck, CorePathsInfo, CorePreflightReport, CoreRuntimeStatus, RuntimePhase,
            ZeytunCore,
        },
        events::EventBus,
        zeytun_core_config::{ZeytunCoreConfigBuilder, SECRET},
    },
};

pub struct CoreLifecycle {
    runtime_status: CoreRuntimeStatus,
    events: Arc<EventBus>,

    compiler: RuntimeProfileCompiler,
    config_builder: ZeytunCoreConfigBuilder,
    process_manager: ZeytunCoreProcessManager,
    system_proxy_manager: SystemProxyManager,
    config_store: ConfigStore,
    clash_api: Option<ClashApiClient>,
}

impl CoreLifecycle {
    pub fn new(
        binary_path: impl Into<std::path::PathBuf>,
        config_dir: impl Into<std::path::PathBuf>,
        events: Arc<EventBus>,
    ) -> Self {
        let binary_path = binary_path.into();
        // Probe the binary once and reuse the output: spawning `zeytun-core version`
        // is a subprocess, and this runs on construction and every reset().
        let version_output = read_binary_version_output(&binary_path);
        let has_tag = |tag: &str| {
            version_output
                .as_deref()
                .map(|output| {
                    output
                        .split([',', '\n', ' '])
                        .any(|part| part.trim() == tag)
                })
                .unwrap_or(false)
        };
        let include_clash_api = has_tag("with_clash_api");
        let include_v2ray_api = has_tag("with_v2ray_api");

        Self {
            runtime_status: CoreRuntimeStatus {
                phase: RuntimePhase::Idle,
                message: None,
                zeytun_core_pid: None,
            },
            events,
            compiler: RuntimeProfileCompiler::new(),
            config_builder: ZeytunCoreConfigBuilder::new(include_clash_api, include_v2ray_api),
            process_manager: ZeytunCoreProcessManager::new(binary_path),
            system_proxy_manager: SystemProxyManager,
            config_store: ConfigStore::new(config_dir.into()),
            clash_api: None,
        }
    }

    pub fn process_manager(&self) -> &ZeytunCoreProcessManager {
        &self.process_manager
    }

    pub fn preflight(&self, binary_path: &Path) -> CorePreflightReport {
        let paths = CorePathsInfo {
            app_data_dir: String::new(),
            resource_dir: None,
            core_dir: String::new(),
            bin_dir: String::new(),
            platform_bin_dir: String::new(),
            runtime_dir: String::new(),
            config_dir: String::new(),
            profile_path: String::new(),
        };

        let (found, executable) = check_binary_path(binary_path);
        let version = if found && executable {
            read_binary_version(binary_path)
        } else {
            None
        };

        let check_error = if !found {
            Some("binary not found".to_string())
        } else if !executable {
            Some("binary is not executable".to_string())
        } else {
            None
        };

        let mut errors = Vec::new();
        if let Some(err) = &check_error {
            errors.push(err.clone());
        }

        let checks = vec![BinaryCheck {
            name: "zeytun-core".to_string(),
            path: Some(binary_path.to_string_lossy().into_owned()),
            found,
            executable,
            required: Some(true),
            version,
            error: check_error,
        }];

        CorePreflightReport {
            ok: errors.is_empty(),
            platform: platform_key(),
            paths,
            checks,
            errors,
        }
    }

    pub fn start(&mut self, profile: &ZeytunCore) -> Result<CoreRuntimeStatus, String> {
        self.runtime_status.phase = RuntimePhase::Starting;
        self.runtime_status.message = Some("starting core".to_string());
        self.runtime_status.zeytun_core_pid = None;

        let profile_model = ProfileConverter::convert(profile);

        let runtime_profile = self
            .compiler
            .compile(&profile_model)
            .map_err(|e| format!("compile failed: {}", e))?;

        let config = self
            .config_builder
            .build(&runtime_profile, &profile.outbound_mode)?;

        let config_path = self
            .config_store
            .write_zeytun_core_config(&runtime_profile.name, &config)?;

        self.process_manager
            .start(config_path.to_string_lossy().as_ref())?;

        self.runtime_status.phase = RuntimePhase::Running;
        self.runtime_status.message = Some("core running".to_string());
        self.runtime_status.zeytun_core_pid = self.process_manager.current_pid()?;

        // Emit lifecycle events for the inbound listeners
        let listen = &runtime_profile.inbound_mode.mixed.listen;
        let listen_port = runtime_profile.inbound_mode.mixed.listen_port;
        let host = if listen.is_empty() {
            "127.0.0.1"
        } else {
            listen
        };

        self.events
            .info("SOCKS5 proxy listening", format!("{host}:{listen_port}"));
        self.events
            .info("HTTP proxy listening", format!("{host}:{listen_port}"));

        let system_proxy_enabled = profile.local_proxy.system_proxy.unwrap_or(false);
        if system_proxy_enabled {
            if let Err(err) = self.system_proxy_manager.enable(host, listen_port) {
                self.events.warning("System proxy failed", err.to_string());
            }
        } else {
            let _ = self.system_proxy_manager.disable();
        }

        if runtime_profile.inbound_mode.tun.is_some() {
            self.events.info("TUN interface active", "");
        }

        if self.config_builder.clash_api_enabled() {
            let listen = self
                .config_builder
                .default_clash_controller(&runtime_profile.inbound_mode);
            let secret = Some(SECRET.to_string());
            self.clash_api = Some(ClashApiClient::new(listen, secret));
        } else {
            self.clash_api = None;
        }

        self.events
            .info("Core started", "zeytun-core process running");

        // Force the clash runtime mode to match the profile's outbound_mode.
        // The core's cache_file may hold a stale mode from a previous session
        // (e.g. "rule") that overrides default_mode at Start. PATCH it now,
        // before any traffic flows, so clash_mode catch-all rules are correct
        // from the first connection.
        if let Some(clash) = self.clash_api.as_ref() {
            let mode_str = serde_json::to_value(profile.outbound_mode)
                .ok()
                .and_then(|v| v.as_str().map(|s| s.to_string()))
                .unwrap_or_else(|| "rule".to_string());
            let _ = clash.patch_config(&mode_str);
        }

        // Live temp overlay (survives only in-core memory; re-push after start).
        // Config also bakes live_rules for cold start; push covers race before seed.
        Self::push_live_rules_to_clash(self.clash_api.as_ref(), profile);

        Ok(self.runtime_status.clone())
    }

    pub fn clash_api(&self) -> Option<&ClashApiClient> {
        self.clash_api.as_ref()
    }

    /// Compile + write config to disk without SIGHUP (live_rules bake for next restart).
    pub fn write_config_only(&mut self, profile: &ZeytunCore) -> Result<(), String> {
        let profile_model = ProfileConverter::convert(profile);
        let runtime_profile = self
            .compiler
            .compile(&profile_model)
            .map_err(|e| format!("compile failed: {}", e))?;
        let config = self
            .config_builder
            .build(&runtime_profile, &profile.outbound_mode)?;
        let _ = self
            .config_store
            .write_zeytun_core_config(&runtime_profile.name, &config)?;
        Ok(())
    }

    pub fn reload(&mut self, profile: &ZeytunCore) -> Result<CoreRuntimeStatus, String> {
        let profile_model = ProfileConverter::convert(profile);

        let runtime_profile = self
            .compiler
            .compile(&profile_model)
            .map_err(|e| format!("compile failed: {}", e))?;

        let config = self
            .config_builder
            .build(&runtime_profile, &profile.outbound_mode)?;

        let config_path = self
            .config_store
            .write_zeytun_core_config(&runtime_profile.name, &config)?;

        self.process_manager
            .reload(config_path.to_string_lossy().as_ref())?;

        if let Some(clash_api) = self.clash_api.clone() {
            std::thread::spawn(move || {
                let _ = clash_api.close_all_connections();
            });
        }

        // SIGHUP recreates router → live_rules seeded from config; push still ok.
        Self::push_live_rules_to_clash(self.clash_api.as_ref(), profile);

        Ok(self.runtime_status.clone())
    }

    fn push_live_rules_to_clash(clash: Option<&ClashApiClient>, profile: &ZeytunCore) {
        let Some(clash) = clash else {
            return;
        };
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0);
        // Temp rules only apply in rule mode (see build_route gate): in
        // Direct/Global push an empty temp overlay so nothing leaks past the
        // clash-mode catch-all (runtime-proven leak). Permanent stays — it
        // matches after system rules and can't leak.
        let temp: Vec<serde_json::Value> =
            if profile.outbound_mode == crate::core::dto::OutboundMode::Rule {
                profile
                    .temp_rules
                    .iter()
                    .filter(|r| {
                        r.enabled
                            && !r.is_expired(now_ms)
                            && !matches!(r.kind, crate::core::dto::RuleType::Final)
                    })
                    .map(|r| {
                        let model = ProfileConverter::core_rule_to_model_pub(&r.to_rule());
                        let rule_json =
                        crate::core::zeytun_core_config::ZeytunCoreConfigBuilder::temp_rule_json(
                            &model,
                        );
                        serde_json::json!({
                            "id": r.id,
                            "expires_at": r.expires_at,
                            "rule": rule_json,
                        })
                    })
                    .collect()
            } else {
                Vec::new()
            };
        let permanent: Vec<serde_json::Value> = profile
            .rules
            .iter()
            .filter(|r| r.enabled && !matches!(r.kind, crate::core::dto::RuleType::Final))
            .map(|r| {
                let model = ProfileConverter::core_rule_to_model_pub(r);
                let rule_json =
                    crate::core::zeytun_core_config::ZeytunCoreConfigBuilder::temp_rule_json(
                        &model,
                    );
                serde_json::json!({
                    "id": r.id,
                    "expires_at": 0,
                    "rule": rule_json,
                })
            })
            .collect();
        let body = serde_json::json!({ "temp": temp, "permanent": permanent }).to_string();
        if let Err(e) = clash.put_live_rules(&body) {
            eprintln!("[live-rules] push failed: {e:?}");
        }
    }

    pub fn set_system_proxy(&self, enabled: bool, profile: &ZeytunCore) -> Result<(), String> {
        if !enabled {
            return self.system_proxy_manager.disable();
        }
        if !self.process_manager.is_running()? {
            return Ok(()); // Saved preference takes effect on the next core start.
        }
        let host = &profile.local_proxy.listen;
        let host = if host.is_empty() { "127.0.0.1" } else { host };
        self.system_proxy_manager
            .enable(host, profile.local_proxy.mixed_port.unwrap_or(6060))
    }

    pub fn stop(&mut self) -> CoreRuntimeStatus {
        self.runtime_status.phase = RuntimePhase::Stopping;
        self.runtime_status.message = Some("stopping core".to_string());

        let _ = self.system_proxy_manager.disable();
        let _ = self.process_manager.stop();

        self.clash_api = None;
        self.runtime_status.phase = RuntimePhase::Idle;
        self.runtime_status.message = Some("core stopped".to_string());
        self.runtime_status.zeytun_core_pid = None;
        self.events.info("Core stopped", "");

        self.runtime_status.clone()
    }

    pub fn status(&mut self) -> CoreRuntimeStatus {
        if matches!(self.runtime_status.phase, RuntimePhase::Running)
            && !self.process_manager.is_running().unwrap_or(false)
        {
            self.runtime_status.phase = RuntimePhase::Idle;
            self.runtime_status.message = Some("process exited".to_string());
            self.runtime_status.zeytun_core_pid = None;
            self.events
                .error("Core stopped unexpectedly", "zeytun-core process exited");
        }

        self.runtime_status.clone()
    }

    pub fn is_running(&self) -> bool {
        matches!(self.runtime_status.phase, RuntimePhase::Running)
    }
}

fn check_binary_path(path: &Path) -> (bool, bool) {
    let found = path.exists();
    let executable = found && is_executable(path);
    (found, executable)
}

fn read_binary_version(path: &Path) -> Option<String> {
    let stdout = read_binary_version_output(path)?;
    let line = stdout.lines().next()?.trim();
    if line.is_empty() {
        None
    } else {
        Some(line.to_string())
    }
}

fn read_binary_version_output(path: &Path) -> Option<String> {
    let output = Command::new(path).arg("version").output().ok()?;
    if !output.status.success() {
        return None;
    }

    Some(String::from_utf8_lossy(&output.stdout).into_owned())
}

fn platform_key() -> String {
    format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH)
}

#[cfg(unix)]
fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(path)
        .map(|meta| meta.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

#[cfg(not(unix))]
fn is_executable(path: &Path) -> bool {
    path.is_file()
}
