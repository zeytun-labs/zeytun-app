// src-tauri/src/adapters/process_resolve.rs
//
// Resolve a rule's process-ish value (PROCESS-NAME / PROCESS-PATH /
// PROCESS-PATH-REGEX) into a display name plus a filesystem path that
// `process_icon::ensure_process_icon` can extract an icon from.
//
// Strategy:
//   - PROCESS-NAME: find an installed .app bundle whose directory name matches
//     (case-insensitive), retrying after stripping common helper suffixes
//     ("Google Chrome Helper (Renderer)" -> "Google Chrome.app").
//   - PROCESS-PATH: if the path points inside a .app bundle, display the
//     bundle's name; otherwise display the binary's file name.
//   - PROCESS-PATH-REGEX: try to match the pattern against installed bundle
//     paths; first match wins.
//
// Bundle locations are scanned once per process lifetime (OnceLock); the
// on-disk icon cache in `process_icon` persists the expensive parts across
// runs.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

#[cfg(target_os = "macos")]
fn app_dirs() -> Vec<PathBuf> {
    let mut dirs = vec![
        PathBuf::from("/Applications"),
        PathBuf::from("/System/Applications"),
    ];
    if let Ok(home) = std::env::var("HOME") {
        dirs.push(PathBuf::from(home).join("Applications"));
    }
    dirs
}

#[cfg(target_os = "windows")]
fn app_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Ok(pf) = std::env::var("ProgramFiles") {
        dirs.push(PathBuf::from(pf));
    }
    if let Ok(pf86) = std::env::var("ProgramFiles(x86)") {
        dirs.push(PathBuf::from(pf86));
    }
    if let Ok(local) = std::env::var("LOCALAPPDATA") {
        dirs.push(PathBuf::from(local).join("Programs"));
    }
    if let Ok(appdata) = std::env::var("APPDATA") {
        dirs.push(PathBuf::from(appdata));
    }
    dirs
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn app_dirs() -> Vec<PathBuf> {
    Vec::new()
}

/// All installed app bundle / executable paths (recursive, bounded depth).
fn app_bundles() -> &'static Vec<PathBuf> {
    static CACHE: OnceLock<Vec<PathBuf>> = OnceLock::new();
    CACHE.get_or_init(|| {
        let mut out = Vec::new();
        for dir in app_dirs() {
            collect_apps(&dir, 0, &mut out);
        }
        out
    })
}

fn collect_apps(dir: &Path, depth: u32, out: &mut Vec<PathBuf>) {
    if depth > 4 {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        #[cfg(target_os = "macos")]
        if path.extension().is_some_and(|e| e == "app") {
            out.push(path);
            continue;
        }
        #[cfg(target_os = "windows")]
        if path
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("exe"))
        {
            out.push(path);
            continue;
        }
        if path.is_dir() {
            collect_apps(&path, depth + 1, out);
        }
    }
}

fn find_bundle_stem<'a>(stem: &str, bundles: &'a [PathBuf]) -> Option<&'a PathBuf> {
    let want = stem.to_lowercase();
    bundles.iter().find(|p| {
        p.file_stem()
            .is_some_and(|s| s.to_string_lossy().to_lowercase() == want)
    })
}

#[cfg(target_os = "windows")]
fn find_in_path(name: &str) -> Option<PathBuf> {
    let name_exe = if name.to_lowercase().ends_with(".exe") {
        name.to_string()
    } else {
        format!("{name}.exe")
    };
    if let Some(paths) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&paths) {
            let candidate = dir.join(&name_exe);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    if let Ok(sysroot) = std::env::var("SystemRoot") {
        let candidate = PathBuf::from(sysroot).join("System32").join(&name_exe);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

/// Helper suffixes, longest first so `strip_suffix` never eats a prefix of a
/// longer suffix (" Helper" before " Helper (Renderer)" would garble the stem).
const HELPER_SUFFIXES: &[&str] = &[
    " Helper (Renderer)",
    " Helper (GPU)",
    " Helper (Plugin)",
    " Helper (Alerts)",
    " Helper",
    " XPC Service",
    " (Intel)",
    " Extension",
    " Agent",
    " Service",
    " Daemon",
];

fn find_bundle<'a>(name: &str, bundles: &'a [PathBuf]) -> Option<&'a PathBuf> {
    let name = name
        .strip_suffix(".app")
        .or_else(|| name.strip_suffix(".exe"))
        .unwrap_or(name);
    if let Some(b) = find_bundle_stem(name, bundles) {
        return Some(b);
    }
    for suffix in HELPER_SUFFIXES {
        if let Some(stem) = name.strip_suffix(suffix) {
            if let Some(b) = find_bundle_stem(stem, bundles) {
                return Some(b);
            }
        }
    }
    None
}

fn bundle_display_name(bundle: &Path) -> String {
    bundle
        .file_stem()
        .unwrap_or(bundle.as_os_str())
        .to_string_lossy()
        .into_owned()
}

/// Extract a readable process name from a regex that didn't resolve to an app
/// bundle. Strips regex escapes/metacharacters and picks the most name-like
/// token: a token ending in ".app" or ".exe" wins, otherwise the longest word.
fn name_from_regex(value: &str) -> String {
    let cleaned = value.replace(r"\.", ".").replace(r"\/", "/");
    let tokens: Vec<String> = cleaned
        .split(|c: char| !c.is_alphanumeric() && c != ' ' && c != '-' && c != '.')
        .map(|t| t.trim_matches(|c: char| !c.is_alphanumeric()).to_string())
        .filter(|t| !t.is_empty())
        .collect();
    if tokens.is_empty() {
        return value.to_string();
    }
    if let Some(t) = tokens
        .iter()
        .find(|t| t.ends_with(".app") || t.ends_with(".exe"))
    {
        let name = t.trim_end_matches(".app").trim_end_matches(".exe");
        if !name.is_empty() {
            return name.to_string();
        }
    }
    tokens
        .iter()
        .max_by_key(|t| t.chars().count())
        .unwrap_or(&tokens[0])
        .to_string()
}

/// Map a rule kind/value to `(display_name, process_path)`. `process_path` is
/// empty when nothing usable was found (icon extraction is skipped then).
pub fn resolve_process(kind: &str, value: &str) -> (String, String) {
    resolve_process_with_bundles(kind, value, || app_bundles())
}

// Keep discovery lazy in production and inject a deterministic inventory in
// unit tests instead of recursively scanning the host's installed programs.
fn resolve_process_with_bundles<'a>(
    kind: &str,
    value: &str,
    bundles: impl Fn() -> &'a [PathBuf],
) -> (String, String) {
    match kind {
        "PROCESS-NAME" => {
            let fallback = value
                .strip_suffix(".app")
                .or_else(|| value.strip_suffix(".exe"))
                .unwrap_or(value)
                .to_string();
            if let Some(bundle) = find_bundle(value, bundles()) {
                return (
                    bundle_display_name(bundle),
                    bundle.to_string_lossy().into_owned(),
                );
            }
            #[cfg(target_os = "windows")]
            if let Some(path) = find_in_path(value) {
                let name = path
                    .file_stem()
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_else(|| fallback.clone());
                return (name, path.to_string_lossy().into_owned());
            }
            (fallback, String::new())
        }
        "PROCESS-PATH" => {
            #[cfg(target_os = "macos")]
            if let Some(idx) = value.find(".app") {
                let bundle = &value[..idx + 4];
                let name = Path::new(bundle)
                    .file_stem()
                    .unwrap_or(Path::new(bundle).as_os_str())
                    .to_string_lossy()
                    .into_owned();
                return (name, bundle.to_string());
            }

            let path = Path::new(value);
            let name = path
                .file_stem()
                .filter(|n| !n.is_empty())
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| value.to_string());
            (name, value.to_string())
        }
        "PROCESS-PATH-REGEX" => {
            if let Ok(re) = regex::Regex::new(value) {
                for bundle in bundles() {
                    let path = bundle.to_string_lossy();
                    if re.is_match(&path) || re.is_match(&format!("{path}/")) {
                        return (
                            bundle_display_name(bundle),
                            bundle.to_string_lossy().into_owned(),
                        );
                    }
                }
            }
            (name_from_regex(value), String::new())
        }
        _ => (value.to_string(), String::new()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn resolve_process(kind: &str, value: &str) -> (String, String) {
        let bundles = [
            PathBuf::from("/Applications/Google Chrome.app"),
            PathBuf::from("/Applications/Telegram.app"),
            PathBuf::from(r"C:\Program Files\Google\Chrome\Application\chrome.exe"),
        ];
        resolve_process_with_bundles(kind, value, || &bundles)
    }

    #[test]
    fn real_rule_values() {
        #[cfg(target_os = "macos")]
        {
            let chrome = "/Applications/Google Chrome.app/Contents/Frameworks/Google Chrome Framework.framework/Versions/149.0.7827.201/Helpers/Google Chrome Helper.app/Contents/MacOS/Google Chrome Helper";
            let (name, path) = resolve_process("PROCESS-PATH", chrome);
            assert_eq!(name, "Google Chrome");
            assert_eq!(path, "/Applications/Google Chrome.app");

            let (name, path) =
                resolve_process("PROCESS-PATH-REGEX", r"^/Applications/Telegram\.app/");
            assert_eq!(name, "Telegram");
            assert_eq!(path, "/Applications/Telegram.app");

            let (name, path) = resolve_process("PROCESS-NAME", "Telegram");
            assert_eq!(name, "Telegram");
            assert_eq!(path, "/Applications/Telegram.app");

            let (name, path) = resolve_process("PROCESS-NAME", "Google Chrome Helper (Renderer)");
            assert_eq!(name, "Google Chrome");
            assert_eq!(path, "/Applications/Google Chrome.app");
        }

        #[cfg(target_os = "windows")]
        {
            let chrome = r"C:\Program Files\Google\Chrome\Application\chrome.exe";
            let (name, path) = resolve_process("PROCESS-PATH", chrome);
            assert_eq!(name, "chrome");
            assert_eq!(path, chrome);
        }
    }

    #[test]
    fn fallbacks() {
        #[cfg(target_os = "windows")]
        {
            let (name, path) = resolve_process("PROCESS-NAME", "cmd");
            assert_eq!(name, "cmd");
            assert!(path.to_lowercase().ends_with("cmd.exe"));
        }

        let (name, path) = resolve_process("PROCESS-PATH-REGEX", r".*nonexistent-app.*");
        assert_eq!(name, "nonexistent-app");
        assert!(path.is_empty());

        let (name, path) = resolve_process("PROCESS-PATH", "/usr/bin/curl");
        assert_eq!(name, "curl");
        assert_eq!(path, "/usr/bin/curl");

        let (name, path) = resolve_process("PROCESS-NAME", "cfprefsd");
        assert_eq!(name, "cfprefsd");
        assert!(path.is_empty());
    }

    #[test]
    fn discovery_is_lazy_for_paths_unknown_rules_and_invalid_regexes() {
        for (kind, value) in [
            ("PROCESS-PATH", "/usr/bin/curl"),
            ("DOMAIN", "example.com"),
            ("PROCESS-PATH-REGEX", "["),
        ] {
            resolve_process_with_bundles(kind, value, || panic!("unnecessary app discovery"));
        }
    }

    #[test]
    fn discovery_scans_a_controlled_directory_tree() {
        let root =
            std::env::temp_dir().join(format!("zeytun-app-discovery-{}", uuid::Uuid::new_v4()));
        let nested = root.join("vendor/version");
        std::fs::create_dir_all(&nested).unwrap();
        #[cfg(target_os = "macos")]
        let app = nested.join("Fixture.app");
        #[cfg(not(target_os = "macos"))]
        let app = nested.join("fixture.exe");
        #[cfg(target_os = "macos")]
        std::fs::create_dir(&app).unwrap();
        #[cfg(not(target_os = "macos"))]
        std::fs::write(&app, b"fixture").unwrap();
        let too_deep = root.join("one/two/three/four/five");
        std::fs::create_dir_all(&too_deep).unwrap();
        #[cfg(target_os = "macos")]
        std::fs::create_dir(too_deep.join("Excluded.app")).unwrap();
        #[cfg(not(target_os = "macos"))]
        std::fs::write(too_deep.join("excluded.exe"), b"fixture").unwrap();
        let mut apps = Vec::new();
        collect_apps(&root, 0, &mut apps);
        std::fs::remove_dir_all(&root).unwrap();
        #[cfg(any(target_os = "macos", target_os = "windows"))]
        assert_eq!(apps, vec![app]);
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        assert!(apps.is_empty());
    }
}
