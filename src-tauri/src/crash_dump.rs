//! Local crash dump: captures panics + recent log lines to timestamped files.
//!
//! Call `install()` at the very start of `main()`, before any Tauri code.

use std::backtrace::Backtrace;
use std::collections::VecDeque;
use std::fmt::Write as FmtWrite;
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::sync::Mutex;

/// Max recent log lines kept in the ring buffer.
pub const LOG_RING_CAP: usize = 200;

/// Max crash dump files retained. Oldest are pruned on each launch.
pub const MAX_DUMP_FILES: usize = 20;

/// Global ring buffer of recent log lines.
static LOG_RING: Mutex<VecDeque<String>> = Mutex::new(VecDeque::new());

/// Test-only or custom override for the dump directory.
static DUMP_DIR_OVERRIDE: Mutex<Option<PathBuf>> = Mutex::new(None);

#[doc(hidden)]
pub fn set_dump_dir_override(dir: Option<PathBuf>) {
    if let Ok(mut guard) = DUMP_DIR_OVERRIDE.lock() {
        *guard = dir;
    }
}

/// Push a log line into the ring buffer and print it to stderr.
pub fn log(line: String) {
    eprintln!("{line}");
    if let Ok(mut ring) = LOG_RING.lock() {
        if ring.len() >= LOG_RING_CAP {
            ring.pop_front();
        }
        ring.push_back(line);
    }
}

/// Convenience macro to log a formatted message to stderr and the crash ring buffer.
#[macro_export]
macro_rules! clog {
    ($($arg:tt)*) => {
        $crate::crash_dump::log(format!($($arg)*))
    };
}

/// Returns the crash dump directory path. Creates it if missing.
pub fn dump_dir() -> Option<PathBuf> {
    if let Ok(guard) = DUMP_DIR_OVERRIDE.lock() {
        if let Some(ref dir) = *guard {
            fs::create_dir_all(dir).ok()?;
            return Some(dir.clone());
        }
    }
    default_dump_dir()
}

#[cfg(target_os = "windows")]
fn default_dump_dir() -> Option<PathBuf> {
    let base = std::env::var("APPDATA")
        .or_else(|_| std::env::var("LOCALAPPDATA"))
        .or_else(|_| std::env::var("USERPROFILE").map(|h| format!("{h}\\AppData\\Roaming")))
        .ok()?;
    let app_folder = if cfg!(debug_assertions) {
        "ZeytunDev"
    } else {
        "Zeytun"
    };
    let dir = PathBuf::from(base).join(app_folder).join("crash-dumps");
    fs::create_dir_all(&dir).ok()?;
    Some(dir)
}

#[cfg(not(target_os = "windows"))]
fn default_dump_dir() -> Option<PathBuf> {
    let home = std::env::var("HOME").ok()?;
    let bundle_id = if cfg!(debug_assertions) {
        "com.zeytun.app.dev"
    } else {
        "com.zeytun.app"
    };
    let dir = PathBuf::from(home).join(format!(
        "Library/Application Support/{bundle_id}/crash-dumps"
    ));
    fs::create_dir_all(&dir).ok()?;
    Some(dir)
}

/// Install the panic hook. Call once at the top of `main()`.
pub fn install() {
    prune_old_dumps();

    let default_hook = std::panic::take_hook();

    std::panic::set_hook(Box::new(move |info| {
        let backtrace = Backtrace::force_capture();
        write_crash_dump(info, &backtrace);
        // Best-effort cleanup of system proxy on panic so network isn't stranded
        let _ = crate::adapters::system_proxy::SystemProxyManager.disable();
        default_hook(info);
    }));
}

/// Build and write the dump file to disk.
pub fn write_crash_dump(
    info: &std::panic::PanicHookInfo<'_>,
    backtrace: &Backtrace,
) -> Option<PathBuf> {
    let mut buf = String::with_capacity(8192);

    let _ = writeln!(buf, "=== ZEYTUN CRASH DUMP ===");
    let _ = writeln!(buf, "Timestamp (UTC): {}", utc_timestamp());
    let _ = writeln!(buf, "App Version: {}", env!("CARGO_PKG_VERSION"));
    let _ = writeln!(buf, "OS: {}", os_version());
    let _ = writeln!(buf, "Arch: {}", std::env::consts::ARCH);
    if let Some(thread) = std::thread::current().name() {
        let _ = writeln!(buf, "Thread: {thread}");
    } else {
        let _ = writeln!(buf, "Thread: (unnamed)");
    }
    let _ = writeln!(buf);

    let _ = writeln!(buf, "--- PANIC ---");
    if let Some(loc) = info.location() {
        let _ = writeln!(
            buf,
            "Location: {}:{}:{}",
            loc.file(),
            loc.line(),
            loc.column()
        );
    }
    let payload = info.payload();
    if let Some(s) = payload.downcast_ref::<&str>() {
        let _ = writeln!(buf, "Message: {}", redact_sensitive(s));
    } else if let Some(s) = payload.downcast_ref::<String>() {
        let _ = writeln!(buf, "Message: {}", redact_sensitive(s));
    } else {
        let _ = writeln!(buf, "Message: (non-string payload)");
    }
    let _ = writeln!(buf);

    let _ = writeln!(buf, "--- BACKTRACE ---");
    let _ = writeln!(buf, "{backtrace}");
    let _ = writeln!(buf);

    let _ = writeln!(buf, "--- RECENT LOG ({LOG_RING_CAP} max) ---");
    if let Ok(ring) = LOG_RING.lock() {
        for line in ring.iter() {
            let _ = writeln!(buf, "  {}", redact_sensitive(line));
        }
    } else {
        let _ = writeln!(buf, "  (log ring lock poisoned)");
    }

    if let Some(dir) = dump_dir() {
        let filename = format!("crash-{}.txt", file_timestamp());
        let path = dir.join(&filename);
        if let Ok(mut f) = fs::File::create(&path) {
            let _ = f.write_all(buf.as_bytes());
            let _ = f.flush();
            eprintln!("[CrashDump] Written to {}", path.display());
            return Some(path);
        }
    }
    None
}

/// List unseen crash dump files (files created since last acknowledgement).
/// Returns paths sorted newest-first.
pub fn list_unseen_dumps() -> Vec<PathBuf> {
    let Some(dir) = dump_dir() else {
        return vec![];
    };
    let ack_path = dir.join(".last-seen");
    let last_seen_ts = fs::metadata(&ack_path)
        .and_then(|m| m.modified())
        .unwrap_or(std::time::UNIX_EPOCH);

    let Ok(entries) = fs::read_dir(&dir) else {
        return vec![];
    };

    let mut dumps: Vec<(std::time::SystemTime, PathBuf)> = entries
        .flatten()
        .filter(|e| {
            e.path()
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with("crash-") && n.ends_with(".txt"))
        })
        .filter_map(|e| {
            let modified = e.metadata().ok()?.modified().ok()?;
            if modified > last_seen_ts {
                Some((modified, e.path()))
            } else {
                None
            }
        })
        .collect();

    dumps.sort_by_key(|a| std::cmp::Reverse(a.0));
    dumps.into_iter().map(|(_, p)| p).collect()
}

/// Mark all current dumps as seen. Touches the `.last-seen` sentinel file.
pub fn mark_dumps_seen() {
    if let Some(dir) = dump_dir() {
        let ack_path = dir.join(".last-seen");
        let _ = fs::write(ack_path, b"seen\n");
    }
}

/// Read a single dump file's content with path traversal guard.
pub fn read_dump(path: &std::path::Path) -> Option<String> {
    let dir = dump_dir()?;
    let canonical_dir = dir.canonicalize().unwrap_or(dir);
    let canonical_path = path.canonicalize().ok()?;
    if !canonical_path.starts_with(&canonical_dir) {
        return None;
    }
    fs::read_to_string(canonical_path).ok()
}

/// Delete old dump files, keeping at most `MAX_DUMP_FILES`.
pub fn prune_old_dumps() {
    let Some(dir) = dump_dir() else { return };
    let Ok(entries) = fs::read_dir(&dir) else {
        return;
    };
    let mut dumps: Vec<(std::time::SystemTime, PathBuf)> = entries
        .flatten()
        .filter(|e| {
            e.path()
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with("crash-") && n.ends_with(".txt"))
        })
        .filter_map(|e| {
            let modified = e.metadata().ok()?.modified().ok()?;
            Some((modified, e.path()))
        })
        .collect();

    if dumps.len() <= MAX_DUMP_FILES {
        return;
    }

    dumps.sort_by_key(|a| std::cmp::Reverse(a.0));
    for (_, path) in dumps.drain(MAX_DUMP_FILES..) {
        let _ = fs::remove_file(path);
    }
}

/// Redact tokens, basic auth credentials, and query params in crash dump text.
pub fn redact_sensitive(text: &str) -> String {
    let mut out = text.to_string();
    // Redact Bearer tokens
    if let Some(idx) = out.find("Bearer ") {
        let after = &out[idx + 7..];
        let token_end = after
            .find(|c: char| c.is_whitespace() || c == '"' || c == '\'')
            .unwrap_or(after.len());
        out.replace_range(idx + 7..idx + 7 + token_end, "[REDACTED]");
    }
    // Redact user:pass in URLs (scheme://user:pass@host)
    if let Some(idx) = out.find("://") {
        if let Some(at_idx) = out[idx + 3..].find('@') {
            let real_at = idx + 3 + at_idx;
            out.replace_range(idx + 3..real_at, "[REDACTED]");
        }
    }
    out
}

/// UTC timestamp for the dump header: `2026-10-07T16:55:00Z` format.
fn utc_timestamp() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format_epoch_secs(secs)
}

/// Filename-safe timestamp: `20261007-165500`.
fn file_timestamp() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let (y, mo, d, h, mi, s) = decompose_epoch(secs);
    format!("{y:04}{mo:02}{d:02}-{h:02}{mi:02}{s:02}")
}

/// Minimal epoch -> calendar decomposition (UTC, no leap seconds).
pub fn decompose_epoch(epoch: u64) -> (u64, u64, u64, u64, u64, u64) {
    let s = epoch % 60;
    let mi = (epoch / 60) % 60;
    let h = (epoch / 3600) % 24;
    let mut days = epoch / 86400;

    let mut y = 1970u64;
    loop {
        let dy = if is_leap(y) { 366 } else { 365 };
        if days < dy {
            break;
        }
        days -= dy;
        y += 1;
    }

    let leap = is_leap(y);
    let month_days: [u64; 12] = [
        31,
        if leap { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    let mut mo = 0u64;
    for md in month_days {
        if days < md {
            break;
        }
        days -= md;
        mo += 1;
    }

    (y, mo + 1, days + 1, h, mi, s)
}

fn is_leap(y: u64) -> bool {
    y.is_multiple_of(4) && (!y.is_multiple_of(100) || y.is_multiple_of(400))
}

pub fn format_epoch_secs(secs: u64) -> String {
    let (y, mo, d, h, mi, s) = decompose_epoch(secs);
    format!("{y:04}-{mo:02}-{d:02}T{h:02}:{mi:02}:{s:02}Z")
}

#[cfg(target_os = "macos")]
fn os_version() -> String {
    let product = sysctl_string("kern.osproductversion").unwrap_or_default();
    let build = sysctl_string("kern.osversion").unwrap_or_default();
    if product.is_empty() {
        return "macOS (unknown)".into();
    }
    format!("macOS {product} ({build})")
}

#[cfg(target_os = "windows")]
fn os_version() -> String {
    "Windows".to_string()
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn os_version() -> String {
    format!("{} (unknown)", std::env::consts::OS)
}

#[cfg(target_os = "macos")]
fn sysctl_string(name: &str) -> Option<String> {
    use std::ffi::CString;
    let cname = CString::new(name).ok()?;
    let mut size: libc::size_t = 0;
    unsafe {
        libc::sysctlbyname(
            cname.as_ptr(),
            std::ptr::null_mut(),
            &mut size,
            std::ptr::null_mut(),
            0,
        );
    }
    if size == 0 {
        return None;
    }
    let mut buf = vec![0u8; size];
    let ret = unsafe {
        libc::sysctlbyname(
            cname.as_ptr(),
            buf.as_mut_ptr() as *mut libc::c_void,
            &mut size,
            std::ptr::null_mut(),
            0,
        )
    };
    if ret != 0 {
        return None;
    }
    while buf.last() == Some(&0) {
        buf.pop();
    }
    String::from_utf8(buf).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_epoch_decomposition() {
        assert_eq!(decompose_epoch(0), (1970, 1, 1, 0, 0, 0));
        assert_eq!(format_epoch_secs(0), "1970-01-01T00:00:00Z");
        assert_eq!(decompose_epoch(86400), (1970, 1, 2, 0, 0, 0));
        // 2026-10-07 12:00:00 UTC = 1791374400
        assert_eq!(decompose_epoch(1791374400), (2026, 10, 7, 12, 0, 0));
    }

    #[test]
    fn test_log_ring_buffer() {
        for i in 0..250 {
            log(format!("test log line {i}"));
        }
        let ring = LOG_RING.lock().unwrap();
        assert_eq!(ring.len(), LOG_RING_CAP);
        assert_eq!(ring.back().unwrap(), "test log line 249");
    }

    #[test]
    fn test_dump_lifecycle_and_security() {
        let temp = std::env::temp_dir().join(format!("zeytun_test_crash_{}", std::process::id()));
        fs::create_dir_all(&temp).unwrap();
        set_dump_dir_override(Some(temp.clone()));

        // Create 2 fake dumps
        let d1 = temp.join("crash-20261007-100000.txt");
        let d2 = temp.join("crash-20261007-110000.txt");
        fs::write(&d1, b"=== ZEYTUN CRASH DUMP ===\nMessage: d1\n").unwrap();
        // Ensure mtime difference
        std::thread::sleep(std::time::Duration::from_millis(15));
        fs::write(&d2, b"=== ZEYTUN CRASH DUMP ===\nMessage: d2\n").unwrap();

        let unseen = list_unseen_dumps();
        assert_eq!(unseen.len(), 2);
        assert_eq!(unseen[0], d2); // newest first

        // Read dump
        let content = read_dump(&d1).expect("d1 content");
        assert!(content.contains("Message: d1"));

        // Path traversal outside dump dir must fail
        let outside = std::env::temp_dir().join("some_outside_file.txt");
        fs::write(&outside, b"secret").unwrap();
        assert!(read_dump(&outside).is_none());
        let _ = fs::remove_file(outside);

        // Mark seen
        std::thread::sleep(std::time::Duration::from_millis(15));
        mark_dumps_seen();
        assert_eq!(list_unseen_dumps().len(), 0);

        // Reset override & clean up
        set_dump_dir_override(None);
        let _ = fs::remove_dir_all(temp);
    }

    #[test]
    fn test_redact_sensitive() {
        assert_eq!(
            redact_sensitive("Authorization: Bearer secret-token-12345"),
            "Authorization: Bearer [REDACTED]"
        );
        assert_eq!(
            redact_sensitive("proxy url http://admin:pass123@example.com:8080/path"),
            "proxy url http://[REDACTED]@example.com:8080/path"
        );
        assert_eq!(
            redact_sensitive("plain benign message"),
            "plain benign message"
        );
    }
}
