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

/// All installed `.app` bundle paths (recursive, bounded depth).
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
        if path.extension().is_some_and(|e| e == "app") {
            out.push(path);
        } else if path.is_dir() {
            collect_apps(&path, depth + 1, out);
        }
    }
}

fn find_bundle_stem(stem: &str) -> Option<&'static PathBuf> {
    let want = stem.to_lowercase();
    app_bundles().iter().find(|p| {
        p.file_stem()
            .is_some_and(|s| s.to_string_lossy().to_lowercase() == want)
    })
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

fn find_bundle(name: &str) -> Option<&'static PathBuf> {
    let name = name.strip_suffix(".app").unwrap_or(name);
    if let Some(b) = find_bundle_stem(name) {
        return Some(b);
    }
    for suffix in HELPER_SUFFIXES {
        if let Some(stem) = name.strip_suffix(suffix) {
            if let Some(b) = find_bundle_stem(stem) {
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
/// token: a token ending in ".app" wins (Telegram from
/// `^/Applications/Telegram\.app/`), otherwise the longest word.
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
    if let Some(t) = tokens.iter().find(|t| t.ends_with(".app")) {
        let name = t.trim_end_matches(".app");
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
    match kind {
        "PROCESS-NAME" => {
            let fallback = value.strip_suffix(".app").unwrap_or(value).to_string();
            match find_bundle(value) {
                Some(bundle) => (
                    bundle_display_name(bundle),
                    bundle.to_string_lossy().into_owned(),
                ),
                None => (fallback, String::new()),
            }
        }
        "PROCESS-PATH" => {
            if let Some(idx) = value.find(".app") {
                let bundle = &value[..idx + 4];
                let name = Path::new(bundle)
                    .file_stem()
                    .unwrap_or(Path::new(bundle).as_os_str())
                    .to_string_lossy()
                    .into_owned();
                (name, bundle.to_string())
            } else {
                let name = Path::new(value)
                    .file_name()
                    .filter(|n| !n.is_empty())
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_else(|| value.to_string());
                (name, value.to_string())
            }
        }
        "PROCESS-PATH-REGEX" => {
            if let Ok(re) = regex::Regex::new(value) {
                // Match the bundle path with and without a trailing slash:
                // `^/Applications/Telegram\.app/` anchors on the slash.
                for bundle in app_bundles() {
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

    #[test]
    fn real_rule_values() {
        let chrome = "/Applications/Google Chrome.app/Contents/Frameworks/Google Chrome Framework.framework/Versions/149.0.7827.201/Helpers/Google Chrome Helper.app/Contents/MacOS/Google Chrome Helper";
        let (name, path) = resolve_process("PROCESS-PATH", chrome);
        assert_eq!(name, "Google Chrome");
        assert_eq!(path, "/Applications/Google Chrome.app");

        let (name, path) = resolve_process("PROCESS-PATH-REGEX", r"^/Applications/Telegram\.app/");
        assert_eq!(name, "Telegram");
        if Path::new("/Applications/Telegram.app").exists() {
            assert_eq!(path, "/Applications/Telegram.app");
        }

        let (name, path) = resolve_process("PROCESS-NAME", "Telegram");
        assert_eq!(name, "Telegram");
        if Path::new("/Applications/Telegram.app").exists() {
            assert_eq!(path, "/Applications/Telegram.app");
        }

        let (name, path) = resolve_process("PROCESS-NAME", "Google Chrome Helper (Renderer)");
        assert_eq!(name, "Google Chrome");
        if Path::new("/Applications/Google Chrome.app").exists() {
            assert_eq!(path, "/Applications/Google Chrome.app");
        }
    }

    #[test]
    fn fallbacks() {
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
}
