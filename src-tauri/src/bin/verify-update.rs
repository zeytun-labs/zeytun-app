//! CI entry point for the same verification used by native updater downloads.
use std::{collections::HashMap, path::PathBuf};
fn run() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let mut options = HashMap::new();
    while let Some(key) = args.next() {
        let value = args
            .next()
            .ok_or("Expected --config, --manifest, --artifacts, --target")?;
        if ![
            "--config",
            "--manifest",
            "--artifacts",
            "--target",
            "--mode",
        ]
        .contains(&key.as_str())
            || options.insert(key, value).is_some()
        {
            return Err("Unknown or duplicate option".into());
        }
    }
    let get = |key: &str| options.get(key).ok_or_else(|| format!("Missing {key}"));
    let read = |path: &str| -> Result<serde_json::Value, String> {
        serde_json::from_slice(&std::fs::read(path).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())
    };
    zeytun_lib::update_security::verify_manifest(
        &read(get("--config")?)?,
        &read(get("--manifest")?)?,
        &PathBuf::from(get("--artifacts")?),
        get("--target")?,
        match options
            .get("--mode")
            .map(String::as_str)
            .unwrap_or("release")
        {
            "release" => false,
            "lab" => true,
            _ => return Err("Mode must be release or lab".into()),
        },
    )?;
    println!("Verified updater signature, release URL, bundle version, identity and architecture");
    Ok(())
}
fn main() {
    if let Err(error) = run() {
        eprintln!("Updater verification failed: {error}");
        std::process::exit(1);
    }
}
