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
const CORE_PROCESS_NAME: &str = if cfg!(target_os = "windows") {
    "zeytun-core.exe"
} else {
    "zeytun-core"
};

#[cfg(target_os = "windows")]
struct JobWrapper(windows::Win32::Foundation::HANDLE);

#[cfg(target_os = "windows")]
unsafe impl Send for JobWrapper {}

#[cfg(target_os = "windows")]
unsafe impl Sync for JobWrapper {}

#[cfg(target_os = "windows")]
impl Drop for JobWrapper {
    fn drop(&mut self) {
        use windows::Win32::Foundation::CloseHandle;
        unsafe {
            let _ = CloseHandle(self.0);
        }
    }
}

pub struct ZeytunCoreProcessManager {
    child: Mutex<Option<Child>>,
    /// Tier 3: handle to the detached `sh` watchdog guarding the current core (Unix).
    watchdog: Mutex<Option<Child>>,
    #[cfg(target_os = "windows")]
    job: Mutex<Option<JobWrapper>>,
    binary_path: PathBuf,
}

impl ZeytunCoreProcessManager {
    pub fn new(binary_path: PathBuf) -> Self {
        Self {
            child: Mutex::new(None),
            watchdog: Mutex::new(None),
            #[cfg(target_os = "windows")]
            job: Mutex::new(None),
            binary_path,
        }
    }

    pub fn start(&self, config_path: &str) -> Result<(), String> {
        self.ensure_setuid(config_path)?;
        self.start_internal(config_path)
    }

    /// Ensure the core binary has sufficient permissions before spawning.
    /// On macOS: verifies/prompts for setuid-root permissions.
    /// On Windows: verifies process elevation if TUN inbound is configured.
    fn ensure_setuid(&self, config_path: &str) -> Result<(), String> {
        #[cfg(unix)]
        {
            let _ = config_path;
            use std::os::unix::fs::MetadataExt;

            let meta = std::fs::metadata(&self.binary_path)
                .map_err(|e| format!("Cannot stat zeytun-core binary: {e}"))?;

            let is_root_owned = meta.uid() == 0;
            let has_setuid = meta.mode() & 0o4000 != 0;

            if is_root_owned && has_setuid {
                return Ok(());
            }

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
                Ok(())
            } else {
                Err(
                    "TUN mode requires root privileges. Please grant permissions when prompted."
                        .to_string(),
                )
            }
        }
        #[cfg(target_os = "windows")]
        {
            let config_content = std::fs::read_to_string(config_path).unwrap_or_default();
            let is_tun = config_content.contains("\"type\": \"tun\"")
                || config_content.contains("\"type\":\"tun\"");

            if is_tun && !is_elevated() {
                return Err(
                    "TUN mode requires Administrator privileges on Windows. Please relaunch Zeytun as Administrator."
                        .to_string(),
                );
            }
            Ok(())
        }
        #[cfg(not(any(unix, target_os = "windows")))]
        {
            let _ = config_path;
            Ok(())
        }
    }

    fn start_internal(&self, config_path: &str) -> Result<(), String> {
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

            let final_message = message;

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

        // ---- Tier 3: launch the watchdog / job object ---------------------
        #[cfg(target_os = "windows")]
        {
            if let Ok(job_handle) = attach_to_job_object(&child) {
                if let Ok(mut jg) = self.job.lock() {
                    *jg = Some(JobWrapper(job_handle));
                }
            }
        }

        // Capture the child PID before moving the handle into managed state,
        // then guard it with a detached watchdog keyed to *this* app's PID.
        let child_pid = child.id();
        *guard = Some(child);
        drop(guard);
        self.spawn_watchdog(child_pid);

        Ok(())
    }

    /// Tier 1: SIGKILL every stray `zeytun-core` currently in the process table
    /// that matches this app's binary path (excluding this app itself).
    /// Best-effort — failures are non-fatal.
    fn kill_orphaned_processes(&self) {
        let self_pid = std::process::id();
        let expected_canonical = std::fs::canonicalize(&self.binary_path).ok();
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

            // Only kill processes matching our binary path (canonicalized).
            // This prevents dev instances from killing a production core or vice-versa.
            let matches = if let (Some(expected), Some(proc_exe)) = (
                &expected_canonical,
                process.exe().and_then(|p| std::fs::canonicalize(p).ok()),
            ) {
                proc_exe == *expected
            } else {
                let name_matches = process
                    .name()
                    .to_str()
                    .map(|n| {
                        n == CORE_PROCESS_NAME
                            || n == "zeytun-core"
                            || n == "zeytun-core.exe"
                            || n == expected_name
                    })
                    .unwrap_or(false);
                let exe_matches = process
                    .exe()
                    .and_then(|p| p.file_name())
                    .and_then(|n| n.to_str())
                    .map(|n| {
                        n == CORE_PROCESS_NAME
                            || n == "zeytun-core"
                            || n == "zeytun-core.exe"
                            || n == expected_name
                    })
                    .unwrap_or(false);
                name_matches || exe_matches
            };

            if matches {
                let _ = process.kill();
            }
        }
    }

    /// Tier 3: spawn a detached `sh` watchdog that kills the core (`kill -9`) as
    /// soon as this app's PID disappears. Survives the app being SIGKILLed /
    /// Force-Quit because it is reparented to launchd. The old watchdog (if any)
    /// is reaped first so only one guards the live core.
    #[cfg(unix)]
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

    #[cfg(not(unix))]
    fn spawn_watchdog(&self, _child_pid: u32) {
        // On Windows, Job Object with JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE
        // kernel-enforces child termination when this parent process dies.
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

        #[cfg(target_os = "windows")]
        if let Ok(mut jg) = self.job.lock() {
            *jg = None;
        }

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
            let _pid = child.id();

            // Send SIGHUP to the core process
            #[cfg(unix)]
            {
                unsafe {
                    libc::kill(_pid as i32, libc::SIGHUP);
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

#[cfg(target_os = "windows")]
fn attach_to_job_object(child: &Child) -> Result<windows::Win32::Foundation::HANDLE, String> {
    use std::os::windows::io::AsRawHandle;
    use windows::Win32::Foundation::{CloseHandle, HANDLE};
    use windows::Win32::System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, JobObjectExtendedLimitInformation,
        SetInformationJobObject, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
        JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    };

    unsafe {
        let job =
            CreateJobObjectW(None, None).map_err(|e| format!("CreateJobObjectW failed: {e}"))?;

        let mut info = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;

        if let Err(e) = SetInformationJobObject(
            job,
            JobObjectExtendedLimitInformation,
            &info as *const _ as *const std::ffi::c_void,
            std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
        ) {
            let _ = CloseHandle(job);
            return Err(format!("SetInformationJobObject failed: {e}"));
        }

        let raw_handle = child.as_raw_handle();
        let target_handle = HANDLE(raw_handle as *mut _);

        if let Err(e) = AssignProcessToJobObject(job, target_handle) {
            let _ = CloseHandle(job);
            return Err(format!("AssignProcessToJobObject failed: {e}"));
        }

        Ok(job)
    }
}

#[cfg(target_os = "windows")]
pub fn is_elevated() -> bool {
    use windows::Win32::Foundation::{CloseHandle, HANDLE};
    use windows::Win32::Security::{
        GetTokenInformation, TokenElevation, TOKEN_ELEVATION, TOKEN_QUERY,
    };
    use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

    unsafe {
        let mut token = HANDLE::default();
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token).is_ok() {
            let mut elevation = TOKEN_ELEVATION::default();
            let mut size = std::mem::size_of::<TOKEN_ELEVATION>() as u32;
            let res = GetTokenInformation(
                token,
                TokenElevation,
                Some(&mut elevation as *mut _ as *mut _),
                size,
                &mut size,
            );
            let _ = CloseHandle(token);
            if res.is_ok() {
                return elevation.TokenIsElevated != 0;
            }
        }
        false
    }
}

#[cfg(not(target_os = "windows"))]
pub fn is_elevated() -> bool {
    true
}

#[cfg(target_os = "windows")]
pub fn relaunch_as_admin() -> Result<(), String> {
    use std::os::windows::ffi::OsStrExt;
    use windows::core::PCWSTR;
    use windows::Win32::UI::Shell::ShellExecuteW;
    use windows::Win32::UI::WindowsAndMessaging::SW_SHOW;

    let current_exe = std::env::current_exe()
        .map_err(|e| format!("Failed to get current executable path: {e}"))?;

    let args: Vec<String> = std::env::args().skip(1).collect();
    let args_string = args
        .iter()
        .map(|a| {
            if a.contains(' ') {
                format!("\"{}\"", a)
            } else {
                a.clone()
            }
        })
        .collect::<Vec<_>>()
        .join(" ");

    let file_wide: Vec<u16> = current_exe
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    let args_wide: Vec<u16> = std::ffi::OsStr::new(&args_string)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    let verb_wide: Vec<u16> = std::ffi::OsStr::new("runas")
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    let cwd_wide: Option<Vec<u16>> = std::env::current_dir().ok().map(|cwd| {
        cwd.as_os_str()
            .encode_wide()
            .chain(std::iter::once(0))
            .collect()
    });

    unsafe {
        let h_instance = ShellExecuteW(
            None,
            PCWSTR(verb_wide.as_ptr()),
            PCWSTR(file_wide.as_ptr()),
            if args.is_empty() {
                PCWSTR::null()
            } else {
                PCWSTR(args_wide.as_ptr())
            },
            cwd_wide
                .as_ref()
                .map(|d| PCWSTR(d.as_ptr()))
                .unwrap_or(PCWSTR::null()),
            SW_SHOW,
        );

        let code = h_instance.0 as isize;
        if code > 32 {
            Ok(())
        } else if code == 5 || code == 1223 {
            Err("Administrator privileges request was cancelled by user.".to_string())
        } else {
            Err(format!("UAC elevation failed (error code {code})."))
        }
    }
}

#[cfg(not(target_os = "windows"))]
pub fn relaunch_as_admin() -> Result<(), String> {
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    #[cfg(target_os = "windows")]
    fn test_is_elevated_callable() {
        // Function executes without panic and returns a boolean
        let _ = super::is_elevated();
    }

    #[test]
    #[cfg(target_os = "windows")]
    fn test_job_object_kills_child_on_close() {
        use std::process::Command;
        use std::time::{Duration, Instant};
        use windows::Win32::Foundation::CloseHandle;

        let mut child = Command::new("powershell.exe")
            .args([
                "-NoLogo",
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                "Start-Sleep -Seconds 30",
            ])
            .spawn()
            .expect("spawn powershell");

        let job = match super::attach_to_job_object(&child) {
            Ok(job) => job,
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                panic!("attach job object: {error}");
            }
        };

        // Closing the job object handle must trigger JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE
        // SAFETY: attach_to_job_object returned an owned, valid handle; close it once.
        let closed = unsafe { CloseHandle(job) };
        let deadline = Instant::now() + Duration::from_secs(5);
        let exited = loop {
            match child.try_wait() {
                Ok(Some(_)) => break Ok(true),
                Ok(None) if Instant::now() < deadline => {
                    std::thread::sleep(Duration::from_millis(20));
                }
                Ok(None) => break Ok(false),
                Err(error) => break Err(error),
            }
        };

        // Reap the child even when closing the job or observing termination fails.
        if !matches!(exited, Ok(true)) {
            let _ = child.kill();
        }
        let reaped = child.wait();
        closed.expect("close job object");
        reaped.expect("wait for child process");
        assert!(
            exited.expect("query child process"),
            "Job Object did not terminate the child within five seconds"
        );
    }
}
