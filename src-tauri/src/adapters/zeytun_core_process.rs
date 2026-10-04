// src-tauri/src/adapters/zeytun_core_process.rs
//
// Three-Tier Defense against orphaned/zombie `zeytun-core` processes:
//
//   Tier 1 — Pre-flight cleanup (sysinfo): before spawning, scan the process
//            table and SIGKILL any stray `zeytun-core` left over from a prior
//            crash / force-quit. Guarantees a clean slate.
//
//   Tier 2 — Graceful shutdown (Drop + explicit stop): the `Child` handle lives
//            in this manager (held in Tauri managed state). `stop()` and the
//            `Drop` impl both `kill()` + `wait()` the core AND the watchdog on a
//            normal shutdown / RunEvent::Exit.
//
//   Tier 3 — Watchdog (force-quit / crash): right after a successful spawn we
//            launch a detached `sh` watchdog that polls the parent (app) PID and
//            `kill -9`s the core the instant the parent dies abruptly — the case
//            Drop can never catch (SIGKILL / Force Quit).

use std::{
    io::Read,
    path::Path,
    path::PathBuf,
    process::{Child, Command, Stdio},
    sync::Mutex,
    thread,
    time::Duration,
};

use sysinfo::{ProcessesToUpdate, System};

/// The exact process name we hunt for in Tier 1. Matches the on-disk binary
/// name resolved in `manager.rs`.
const CORE_PROCESS_NAME: &str = "zeytun-core";

pub struct ZeytunCoreProcessManager {
    child: Mutex<Option<Child>>,
    /// Tier 3: handle to the detached `sh` watchdog guarding the current core.
    watchdog: Mutex<Option<Child>>,
    binary_path: PathBuf,
}

impl ZeytunCoreProcessManager {
    pub fn new(binary_path: PathBuf) -> Self {
        Self {
            child: Mutex::new(None),
            watchdog: Mutex::new(None),
            binary_path,
        }
    }

    pub fn start(&self, config_path: &str) -> Result<(), String> {
        self.start_internal(config_path, false)
    }

    fn start_internal(&self, config_path: &str, is_retry: bool) -> Result<(), String> {
        // Always stop existing child first — never early-return while still running
        // (that skipped new config after retry_rulesets write).
        {
            let mut guard = self.child.lock().map_err(|e| e.to_string())?;
            if let Some(mut child) = guard.take() {
                self.stop_watchdog();
                let _ = child.kill();
                let _ = child.wait();
            }
        }

        self.check_config(config_path)?;

        // ---- Tier 1: pre-flight cleanup ------------------------------------
        // Kill any orphaned cores from a previous run before spawning a new one.
        self.kill_orphaned_processes();

        // cwd = config dir so experimental.cache_file path "cache.db" lands next to config.
        let mut cmd = Command::new(&self.binary_path);
        cmd.arg("run").arg("-c").arg(config_path);
        if let Some(dir) = std::path::Path::new(config_path).parent() {
            cmd.current_dir(dir);
        }
        let mut child = cmd
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("Failed to start zeytun-core: {e}"))?;

        thread::sleep(Duration::from_millis(250));

        let mut guard = self.child.lock().map_err(|e| e.to_string())?;

        if let Some(status) = child.try_wait().map_err(|e| e.to_string())? {
            let stdout = read_pipe(child.stdout.take());
            let stderr = read_pipe(child.stderr.take());
            let message = [stderr.trim(), stdout.trim()]
                .into_iter()
                .filter(|part| !part.is_empty())
                .collect::<Vec<_>>()
                .join("\n");

            let mut final_message = message.clone();
            if final_message.contains("configure tun interface: Connect: operation not permitted") {
                if !is_retry {
                    // Drop the guard before retrying to prevent deadlocks
                    drop(guard);

                    let binary_str = self.binary_path.to_string_lossy();
                    let script = format!(
                        "do shell script \"chown root '{binary_str}' && chmod 4755 '{binary_str}'\" with administrator privileges"
                    );

                    let output = Command::new("osascript")
                        .arg("-e")
                        .arg(&script)
                        .output()
                        .map_err(|e| format!("Failed to execute osascript: {e}"))?;

                    if output.status.success() {
                        return self.start_internal(config_path, true);
                    }
                }

                final_message = "TUN mode requires Administrator/Root privileges on macOS. Please grant permissions when prompted.".to_string();
            }

            return Err(format!(
                "zeytun-core exited during startup ({status}): {}",
                if final_message.is_empty() {
                    "no output".to_string()
                } else {
                    final_message
                }
            ));
        }

        drain_pipe(child.stdout.take());
        drain_pipe(child.stderr.take());

        // ---- Tier 3: launch the watchdog -----------------------------------
        // Capture the child PID before moving the handle into managed state,
        // then guard it with a detached watchdog keyed to *this* app's PID.
        let child_pid = child.id();
        *guard = Some(child);
        drop(guard);
        self.spawn_watchdog(child_pid);

        Ok(())
    }

    /// Tier 1: SIGKILL every stray `zeytun-core` currently in the process table
    /// (excluding this app itself). Best-effort — failures are non-fatal.
    fn kill_orphaned_processes(&self) {
        let self_pid = std::process::id();
        let expected_name = self
            .binary_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(CORE_PROCESS_NAME);

        let mut system = System::new();
        system.refresh_processes(ProcessesToUpdate::All, true);

        for (pid, process) in system.processes() {
            // Never target ourselves.
            if pid.as_u32() == self_pid {
                continue;
            }

            // Match on the reported process name OR the executable's file name.
            // (`name()` can be truncated on some platforms, so we check both.)
            let name_matches = process
                .name()
                .to_str()
                .map(|n| n == CORE_PROCESS_NAME || n == expected_name)
                .unwrap_or(false);
            let exe_matches = process
                .exe()
                .and_then(|p| p.file_name())
                .and_then(|n| n.to_str())
                .map(|n| n == CORE_PROCESS_NAME || n == expected_name)
                .unwrap_or(false);

            if name_matches || exe_matches {
                let _ = process.kill();
            }
        }
    }

    /// Tier 3: spawn a detached `sh` watchdog that kills the core (`kill -9`) as
    /// soon as this app's PID disappears. Survives the app being SIGKILLed /
    /// Force-Quit because it is reparented to launchd. The old watchdog (if any)
    /// is reaped first so only one guards the live core.
    fn spawn_watchdog(&self, child_pid: u32) {
        let parent_pid = std::process::id();
        let watchdog_script = format!(
            "while kill -0 {parent_pid} 2>/dev/null; do sleep 1; done; kill -9 {child_pid}"
        );

        let spawned = Command::new("sh")
            .arg("-c")
            .arg(&watchdog_script)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn();

        if let Ok(watchdog) = spawned {
            if let Ok(mut guard) = self.watchdog.lock() {
                // Reap any previous watchdog before replacing it.
                if let Some(mut old) = guard.take() {
                    let _ = old.kill();
                    let _ = old.wait();
                }
                *guard = Some(watchdog);
            }
        }
    }

    /// Kill + reap the current watchdog (Tier 2 cleanup companion to `stop`).
    fn stop_watchdog(&self) {
        if let Ok(mut guard) = self.watchdog.lock() {
            if let Some(mut watchdog) = guard.take() {
                let _ = watchdog.kill();
                let _ = watchdog.wait();
            }
        }
    }

    pub fn stop(&self) -> Result<(), String> {
        // Tier 2: tear the watchdog down first so it can't `kill -9` a PID that
        // may be reused after we reap the core.
        self.stop_watchdog();

        let mut guard = self.child.lock().map_err(|e| e.to_string())?;

        if let Some(mut child) = guard.take() {
            let _ = child.kill();
            let _ = child.wait();
        }

        Ok(())
    }

    pub fn reload(&self, config_path: &str) -> Result<(), String> {
        self.check_config(config_path)?;

        let guard = self.child.lock().map_err(|e| e.to_string())?;
        if let Some(child) = guard.as_ref() {
            let pid = child.id();

            // Send SIGHUP to the core process
            #[cfg(unix)]
            {
                unsafe {
                    libc::kill(pid as i32, libc::SIGHUP);
                }
            }
            #[cfg(windows)]
            {
                // Fallback to restart on Windows since SIGHUP is UNIX-only
                drop(guard);
                return self.restart(config_path);
            }
        }

        Ok(())
    }

    pub fn restart(&self, config_path: &str) -> Result<(), String> {
        self.stop()?;
        self.start(config_path)
    }

    pub fn is_running(&self) -> Result<bool, String> {
        let mut guard = self.child.lock().map_err(|e| e.to_string())?;
        let Some(child) = guard.as_mut() else {
            return Ok(false);
        };

        match child.try_wait() {
            Ok(Some(_)) => {
                *guard = None;
                Ok(false)
            }
            Ok(None) => Ok(true),
            Err(err) => Err(err.to_string()),
        }
    }

    pub fn current_pid(&self) -> Result<Option<u32>, String> {
        let guard = self.child.lock().map_err(|e| e.to_string())?;
        Ok(guard.as_ref().map(|child| child.id()))
    }

    pub fn binary_path(&self) -> &Path {
        &self.binary_path
    }

    fn check_config(&self, config_path: &str) -> Result<(), String> {
        let output = Command::new(&self.binary_path)
            .arg("check")
            .arg("-c")
            .arg(config_path)
            .output()
            .map_err(|e| format!("Failed to check zeytun-core config: {e}"))?;

        if output.status.success() {
            return Ok(());
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        let message = [stderr.trim(), stdout.trim()]
            .into_iter()
            .filter(|part| !part.is_empty())
            .collect::<Vec<_>>()
            .join("\n");

        Err(format!(
            "zeytun-core config check failed: {}",
            if message.is_empty() {
                output.status.to_string()
            } else {
                message
            }
        ))
    }
}

impl Drop for ZeytunCoreProcessManager {
    fn drop(&mut self) {
        // Tier 2: normal-shutdown safety net. Reap the watchdog first, then the
        // core, so no `kill -9` fires against a recycled PID.
        self.stop_watchdog();

        if let Ok(mut guard) = self.child.lock() {
            if let Some(mut child) = guard.take() {
                let _ = child.kill();
                let _ = child.wait();
            }
        }
    }
}

fn read_pipe<T: Read>(pipe: Option<T>) -> String {
    let Some(mut pipe) = pipe else {
        return String::new();
    };

    let mut output = String::new();
    let _ = pipe.read_to_string(&mut output);
    output
}

fn drain_pipe<T>(pipe: Option<T>)
where
    T: Read + Send + 'static,
{
    if let Some(mut pipe) = pipe {
        thread::spawn(move || {
            let mut buffer = [0_u8; 4096];
            while matches!(pipe.read(&mut buffer), Ok(count) if count > 0) {}
        });
    }
}
