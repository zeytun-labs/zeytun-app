// src-tauri/src/adapters/privileged_core.rs
//
// macOS TUN needs zeytun-core to run as root. Until the launchd helper
// (docs/macos-privileged-core.md) lands, the core is a setuid-root binary, so
// this module keeps everything a root process touches out of the user's reach:
//
//   * The setuid copy lives in a root-owned directory
//     (`/Library/Application Support/Zeytun/bin`), never in app data.
//   * It is copied from the app bundle *as the user* into a root-owned temp
//     file, and its SHA-256 is checked against the bundled copy before any
//     chown/chmod. A file swapped in mid-install never gets the setuid bit.
//   * The config it runs is staged into a root-owned directory
//     (`.../Zeytun/run/config.json`, root:wheel 0600). zeytun-core refuses to
//     run as root on any config that is not root-owned and non-writable by
//     group/other, so a user-written config can no longer drive the root core.
//
// Non-TUN sessions never use this path: they run the unprivileged app-data
// copy as the user.

use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
    sync::OnceLock,
};

use sha2::{Digest, Sha256};

/// Root-owned install prefix for the privileged core and its staged config.
#[cfg(target_os = "macos")]
pub const PRIVILEGED_ROOT: &str = "/Library/Application Support/Zeytun";

static BUNDLED_CORE: OnceLock<PathBuf> = OnceLock::new();

/// Remember where the app bundle's zeytun-core lives. Only this copy is ever
/// promoted to root; the app-data copy is user-writable.
pub fn set_bundled_core_source(path: &Path) {
    let _ = BUNDLED_CORE.set(path.to_path_buf());
}

pub fn bundled_core_source() -> Option<&'static Path> {
    BUNDLED_CORE.get().map(PathBuf::as_path)
}

/// Lowercase hex SHA-256 of a file, streamed.
pub fn sha256_file(path: &Path) -> Result<String, String> {
    let mut file =
        fs::File::open(path).map_err(|e| format!("Cannot open {}: {e}", path.display()))?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 64 * 1024];
    loop {
        let n = file
            .read(&mut buf)
            .map_err(|e| format!("Cannot read {}: {e}", path.display()))?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hasher
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect())
}

/// Ok when `path` hashes to `expected` (lowercase hex SHA-256).
pub fn verify_sha256(path: &Path, expected: &str) -> Result<(), String> {
    let actual = sha256_file(path)?;
    if actual.eq_ignore_ascii_case(expected) {
        Ok(())
    } else {
        Err(format!(
            "SHA-256 mismatch for {}: expected {expected}, got {actual}",
            path.display()
        ))
    }
}

/// The ownership rule a root-executed core (or any directory above it) must
/// satisfy: owned by root, not writable by group or other. `setuid` asks for
/// the setuid bit as well.
pub fn is_trusted_root_file(uid: u32, mode: u32, setuid: bool) -> bool {
    uid == 0 && mode & 0o022 == 0 && (!setuid || mode & 0o4000 != 0)
}

/// True when `installed` is a root-owned setuid binary, not writable by
/// group/other, whose bytes match `expected_sha256`.
#[cfg(unix)]
pub fn installed_core_is_trusted(installed: &Path, expected_sha256: &str) -> bool {
    use std::os::unix::fs::MetadataExt;
    let Ok(meta) = fs::symlink_metadata(installed) else {
        return false;
    };
    meta.file_type().is_file()
        && is_trusted_root_file(meta.uid(), meta.mode(), true)
        && verify_sha256(installed, expected_sha256).is_ok()
}

/// Delete setuid binaries left in a user-writable directory by older builds
/// (which made the app-data copy setuid-root). Their directory belongs to the
/// user, so unlinking works without privileges. Returns what was removed.
#[cfg(unix)]
pub fn remove_stale_setuid_cores(dir: &Path) -> Vec<PathBuf> {
    use std::os::unix::fs::MetadataExt;
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut removed = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        let is_core = entry
            .file_name()
            .to_str()
            .is_some_and(|name| name.starts_with("zeytun-core"));
        let is_setuid = fs::symlink_metadata(&path)
            .is_ok_and(|meta| meta.file_type().is_file() && meta.mode() & 0o6000 != 0);
        if is_core && is_setuid && fs::remove_file(&path).is_ok() {
            removed.push(path);
        }
    }
    removed
}

/// Delete files in the unprivileged runtime config directory that the user
/// no longer owns. Older builds ran the setuid core there, so root created
/// `cache.db`; the unprivileged core then fails at startup with
/// "open cache.db: permission denied". These are caches the core recreates,
/// and the directory is the user's, so unlinking needs no privilege. Only
/// regular files directly in `dir` are touched. Returns what was removed.
#[cfg(unix)]
pub fn remove_foreign_owned_files(dir: &Path) -> Vec<PathBuf> {
    use std::os::unix::fs::MetadataExt;
    let own_uid = unsafe { libc::getuid() };
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut removed = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        let foreign = fs::symlink_metadata(&path)
            .is_ok_and(|meta| meta.file_type().is_file() && meta.uid() != own_uid);
        if foreign && fs::remove_file(&path).is_ok() {
            removed.push(path);
        }
    }
    removed
}

#[cfg(target_os = "macos")]
pub fn privileged_core_path() -> PathBuf {
    Path::new(PRIVILEGED_ROOT)
        .join("bin")
        .join(format!("zeytun-core-{}", env!("CARGO_PKG_VERSION")))
}

#[cfg(target_os = "macos")]
pub fn staged_config_path() -> PathBuf {
    Path::new(PRIVILEGED_ROOT).join("run").join("config.json")
}

/// SHA-256 of the staged config, written by the privileged script (root-owned,
/// world-readable). The staged config itself is 0600, so this is how the app
/// learns what root is currently running without another prompt.
#[cfg(target_os = "macos")]
fn staged_config_hash_path() -> PathBuf {
    Path::new(PRIVILEGED_ROOT).join("run").join("config.sha256")
}

/// SHA-256 of the bundled core, baked in by `build.rs` from
/// `resources/bin/<platform>/zeytun-core`. It lives inside the code-signed app
/// binary, so replacing the core inside the .app cannot also replace the hash
/// it is checked against.
pub const EMBEDDED_CORE_SHA256: Option<&str> = option_env!("ZEYTUN_CORE_SHA256");

/// The hash the privileged core must match. Release builds without an
/// embedded hash refuse TUN rather than trust the file on disk; debug builds
/// (no native resources at compile time) fall back to hashing the bundle.
pub fn expected_core_sha256(bundled: &Path) -> Result<String, String> {
    match EMBEDDED_CORE_SHA256 {
        Some(hash) => Ok(hash.to_ascii_lowercase()),
        None if cfg!(debug_assertions) => sha256_file(bundled),
        None => Err(
            "This build has no embedded zeytun-core hash, so TUN mode is disabled. Reinstall Zeytun."
                .to_string(),
        ),
    }
}

/// True when root has already staged exactly `config_sha256`: the marker
/// and staged config are root-owned and not writable by group/other, and the
/// marker matches. Only root can write either file, so a match means root is
/// set up to run this content and no new prompt is needed.
#[cfg(target_os = "macos")]
fn staged_config_matches(config_sha256: &str) -> bool {
    use std::os::unix::fs::MetadataExt;
    let trusted = |path: &Path| {
        fs::symlink_metadata(path).is_ok_and(|meta| {
            meta.file_type().is_file() && is_trusted_root_file(meta.uid(), meta.mode(), false)
        })
    };
    let marker = staged_config_hash_path();
    trusted(&staged_config_path())
        && trusted(&marker)
        && fs::read_to_string(&marker)
            .is_ok_and(|stored| stored.trim().eq_ignore_ascii_case(config_sha256))
}

/// Runs as root inside one admin prompt. Arguments arrive as positional
/// parameters, never spliced into the script text:
///   $1 caller uid   $2 bundled core   $3 expected sha256 ("-" = keep core)
///   $4 user config  $5 root prefix    $6 core file name
/// Sources are read with `sudo -u` as the caller, so a symlink in a user path
/// cannot make root copy a file the user could not read themselves.
#[cfg(target_os = "macos")]
const PRIVILEGED_SCRIPT: &str = r##"set -eu
umask 077
uid="$1"; src_core="$2"; expected="$3"; src_cfg="$4"; root="$5"; core_name="$6"
for d in "$root" "$root/bin" "$root/run"; do
  if [ -L "$d" ]; then echo "refusing symlinked $d" >&2; exit 2; fi
  mkdir -p "$d"; chown root:wheel "$d"; chmod 755 "$d"
done
if [ "$expected" != "-" ]; then
  tmp=$(mktemp "$root/bin/.zeytun-core.XXXXXX")
  /usr/bin/sudo -n -u "#$uid" /bin/cat -- "$src_core" > "$tmp"
  actual=$(/usr/bin/shasum -a 256 "$tmp" | /usr/bin/cut -d' ' -f1)
  if [ "$actual" != "$expected" ]; then
    rm -f "$tmp"; echo "zeytun-core SHA-256 mismatch: refusing to install" >&2; exit 3
  fi
  chown root:wheel "$tmp"; chmod 4755 "$tmp"; mv -f "$tmp" "$root/bin/$core_name"
  for old in "$root"/bin/zeytun-core-*; do
    if [ "$old" != "$root/bin/$core_name" ]; then rm -f "$old"; fi
  done
fi
tmp=$(mktemp "$root/run/.config.XXXXXX")
/usr/bin/sudo -n -u "#$uid" /bin/cat -- "$src_cfg" > "$tmp"
chown root:wheel "$tmp"; chmod 600 "$tmp"
sum=$(mktemp "$root/run/.config-sha.XXXXXX")
/usr/bin/shasum -a 256 "$tmp" | /usr/bin/cut -d' ' -f1 > "$sum"
chown root:wheel "$sum"; chmod 644 "$sum"
mv -f "$tmp" "$root/run/config.json"
mv -f "$sum" "$root/run/config.sha256"
"##;

/// Make sure the root-owned core is installed and current, and stage
/// `user_config` next to it. Shows the macOS admin prompt. Returns the paths
/// to spawn: (privileged core, staged config).
#[cfg(target_os = "macos")]
pub fn prepare_privileged_launch(user_config: &Path) -> Result<(PathBuf, PathBuf), String> {
    use std::process::Command;

    let bundled = bundled_core_source()
        .ok_or("Bundled zeytun-core not found; cannot install the privileged core")?;
    let expected = expected_core_sha256(bundled)?;
    let core = privileged_core_path();
    let core_trusted = installed_core_is_trusted(&core, &expected);

    // Nothing to do when root already runs this exact config on a trusted
    // core: no prompt for an unchanged config (restart, login autostart).
    let config_sha256 = sha256_file(user_config)?;
    if core_trusted && staged_config_matches(&config_sha256) {
        return Ok((core, staged_config_path()));
    }

    let core_arg = if core_trusted {
        "-".to_string()
    } else {
        expected.clone()
    };
    let core_name = core
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or("invalid privileged core path")?
        .to_string();
    let uid = unsafe { libc::getuid() }.to_string();

    let output = Command::new("osascript")
        .args([
            "-e",
            "on run argv",
            "-e",
            "set cmd to \"/bin/sh -c \" & quoted form of (item 1 of argv) & \" zeytun-privileged\"",
            "-e",
            "repeat with i from 2 to count of argv",
            "-e",
            "set cmd to cmd & \" \" & quoted form of (item i of argv)",
            "-e",
            "end repeat",
            "-e",
            "do shell script cmd with administrator privileges",
            "-e",
            "end run",
        ])
        .arg(PRIVILEGED_SCRIPT)
        .arg(&uid)
        .arg(bundled)
        .arg(&core_arg)
        .arg(user_config)
        .arg(PRIVILEGED_ROOT)
        .arg(&core_name)
        .output()
        .map_err(|e| format!("Failed to execute osascript: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if stderr.contains("SHA-256 mismatch") {
            return Err(
                "The bundled zeytun-core changed during install and was not trusted. Reinstall Zeytun."
                    .to_string(),
            );
        }
        return Err(
            "TUN mode requires administrator approval. Please grant permissions when prompted."
                .to_string(),
        );
    }

    // Belt and braces: never spawn a root core we cannot vouch for, and
    // confirm root staged the content we validated (not a file swapped in
    // between our hash and its copy).
    if !installed_core_is_trusted(&core, &expected) {
        return Err("Privileged zeytun-core failed verification after install".to_string());
    }
    if !staged_config_matches(&config_sha256) {
        return Err("The core config changed while it was being staged. Try again.".to_string());
    }
    Ok((core, staged_config_path()))
}

#[cfg(test)]
mod tests {
    use super::*;

    struct ScratchDir(PathBuf);

    impl ScratchDir {
        fn new() -> Self {
            let dir = std::env::temp_dir()
                .join(format!("zeytun-privileged-core-{}", uuid::Uuid::new_v4()));
            fs::create_dir_all(&dir).unwrap();
            Self(dir)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for ScratchDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    // SHA-256("abc"), FIPS 180-2 test vector.
    const ABC_SHA256: &str = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";

    #[test]
    fn sha256_matches_known_vector() {
        let dir = ScratchDir::new();
        let path = dir.path().join("core");
        fs::write(&path, b"abc").unwrap();
        assert_eq!(sha256_file(&path).unwrap(), ABC_SHA256);
    }

    #[test]
    fn verify_sha256_rejects_tampered_binary() {
        let dir = ScratchDir::new();
        let bundled = dir.path().join("bundled");
        let planted = dir.path().join("planted");
        fs::write(&bundled, b"real core").unwrap();
        fs::write(&planted, b"real core + payload").unwrap();

        let expected = sha256_file(&bundled).unwrap();
        assert!(verify_sha256(&bundled, &expected).is_ok());
        assert!(verify_sha256(&bundled, &expected.to_uppercase()).is_ok());
        let err = verify_sha256(&planted, &expected).unwrap_err();
        assert!(err.contains("mismatch"), "{err}");
    }

    #[test]
    fn verify_sha256_errors_on_missing_file() {
        let dir = ScratchDir::new();
        assert!(verify_sha256(&dir.path().join("missing"), ABC_SHA256).is_err());
    }

    #[test]
    fn expected_hash_prefers_embedded_value() {
        let dir = ScratchDir::new();
        let bundled = dir.path().join("core");
        fs::write(&bundled, b"abc").unwrap();
        let expected = expected_core_sha256(&bundled).unwrap();
        match EMBEDDED_CORE_SHA256 {
            // The on-disk file must not be able to change the expected hash.
            Some(embedded) => assert_eq!(expected, embedded.to_ascii_lowercase()),
            None => assert_eq!(expected, ABC_SHA256),
        }
    }

    #[cfg(unix)]
    #[test]
    fn keeps_files_the_user_owns() {
        let dir = ScratchDir::new();
        let cache = dir.path().join("cache.db");
        fs::write(&cache, b"db").unwrap();
        assert!(remove_foreign_owned_files(dir.path()).is_empty());
        assert!(cache.exists());
        assert!(remove_foreign_owned_files(&dir.path().join("missing")).is_empty());
    }

    #[test]
    fn trusted_root_file_rule() {
        assert!(is_trusted_root_file(0, 0o104755, true));
        assert!(is_trusted_root_file(0, 0o100600, false));
        // Not root-owned.
        assert!(!is_trusted_root_file(501, 0o104755, true));
        // Group- or world-writable.
        assert!(!is_trusted_root_file(0, 0o104775, true));
        assert!(!is_trusted_root_file(0, 0o104757, true));
        // setuid required but missing.
        assert!(!is_trusted_root_file(0, 0o100755, true));
    }

    #[cfg(unix)]
    #[test]
    fn user_owned_core_is_never_trusted() {
        let dir = ScratchDir::new();
        let core = dir.path().join("zeytun-core");
        fs::write(&core, b"abc").unwrap();
        // Right bytes, wrong owner: a planted copy in a user directory must
        // not pass even when its hash matches.
        assert!(!installed_core_is_trusted(&core, ABC_SHA256));
        assert!(!installed_core_is_trusted(
            &dir.path().join("missing"),
            ABC_SHA256
        ));
    }

    #[cfg(unix)]
    #[test]
    fn removes_only_setuid_core_leftovers() {
        use std::os::unix::fs::PermissionsExt;
        let dir = ScratchDir::new();
        let stale = dir.path().join("zeytun-core-0.0.9");
        let current = dir.path().join("zeytun-core-0.1.0");
        let other = dir.path().join("helper");
        for path in [&stale, &current, &other] {
            fs::write(path, b"bin").unwrap();
        }
        fs::set_permissions(&stale, fs::Permissions::from_mode(0o4755)).unwrap();
        fs::set_permissions(&current, fs::Permissions::from_mode(0o755)).unwrap();
        fs::set_permissions(&other, fs::Permissions::from_mode(0o4755)).unwrap();

        let removed = remove_stale_setuid_cores(dir.path());
        assert_eq!(removed, vec![stale.clone()]);
        assert!(!stale.exists());
        assert!(current.exists());
        assert!(other.exists());
    }
}
