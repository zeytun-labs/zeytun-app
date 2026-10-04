//! Guards the Tauri command registry against the bug class where a command is
//! defined with `#[tauri::command]` but never added to `generate_handler!` — it
//! compiles fine and only fails at runtime with "unknown command" (the Log
//! Level setting shipped broken this way).
//!
//! The test parses lib.rs directly: a matching pair of `#[tauri::command]` +
//! `async fn <name>` must also appear inside the `generate_handler![...]`
//! macro invocation.

use std::path::Path;

#[test]
fn every_tauri_command_is_registered() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let src = std::fs::read_to_string(manifest.join("src/lib.rs")).expect("read src/lib.rs");

    // 1. Commands: `#[tauri::command]` followed (ignoring attributes/doc
    //    comments) by `async fn <name>(` or `fn <name>(`.
    let mut defined: Vec<String> = Vec::new();
    let mut saw_command_attr = false;
    for line in src.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("#[tauri::command]") {
            saw_command_attr = true;
            continue;
        }
        if saw_command_attr {
            if trimmed.is_empty() || trimmed.starts_with("//") || trimmed.starts_with("#[") {
                continue; // attribute/doc/blank between attr and fn
            }
            if let Some(name) = trimmed
                .trim_start_matches("pub ")
                .strip_prefix("async fn ")
                .or_else(|| trimmed.trim_start_matches("pub ").strip_prefix("fn "))
            {
                if let Some(name) = name.split(['(', '<']).next() {
                    defined.push(name.trim().to_string());
                }
            }
            saw_command_attr = false;
        }
    }
    assert!(
        !defined.is_empty(),
        "parser found no #[tauri::command] fns — if the file layout changed, update this test"
    );

    // 2. Registered names inside generate_handler![...].
    let marker = "generate_handler![";
    let handler_start = src
        .find(marker)
        .expect("generate_handler! block missing from lib.rs");
    let block_start = handler_start + marker.len();
    let handler_end = src[block_start..]
        .find("]")
        .expect("generate_handler! block not terminated");
    let block = &src[block_start..block_start + handler_end];
    let registered: Vec<String> = block
        .split(',')
        .map(|s| {
            s.trim()
                .trim_start_matches("crate::commands::")
                .trim_start_matches("self::")
                .to_string()
        })
        .filter(|s| !s.is_empty())
        .collect();

    // 3. Every defined command must be registered.
    let mut missing: Vec<&String> = defined
        .iter()
        .filter(|name| !registered.iter().any(|r| r == *name))
        .collect();
    missing.sort();
    missing.dedup();
    assert!(
        missing.is_empty(),
        "commands defined with #[tauri::command] but NOT in generate_handler! \
         (they would fail at runtime with 'unknown command'): {missing:?}"
    );
}
