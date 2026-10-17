//! One trust policy for native installation and release verification. No renderer
//! can supply a public key, target version, identity, endpoint, or raw bytes.
use base64::Engine;
#[cfg(target_os = "macos")]
use flate2::read::GzDecoder;
use semver::Version;
use serde_json::Value;
#[cfg(target_os = "macos")]
use std::{
    collections::{HashMap, HashSet},
    io::{Cursor, Read},
    path::Component,
};
use std::path::Path;
use url::Url;

pub const REPOSITORY: &str = "zeytun-labs/zeytun-app";
pub const PRODUCTION_ID: &str = "com.zeytun.app";
pub const LAB_ID: &str = "com.zeytun.app.lab";
#[cfg(target_os = "macos")]
const MAX_METADATA: u64 = 1024 * 1024;
#[cfg(target_os = "macos")]
const MAX_EXPANDED: u64 = 4 * 1024 * 1024 * 1024;
#[cfg(target_os = "macos")]
const MAX_ENTRIES: usize = 100_000;

pub type Result<T> = std::result::Result<T, String>;

#[derive(Clone, Debug)]
pub struct BundleRequirements {
    pub version: Version,
    pub identifier: String,
    pub arch: String,
}

/// Construction is private to successful authentication and metadata checks.
pub struct VerifiedArchive(Vec<u8>);
impl VerifiedArchive {
    pub fn bytes(&self) -> &[u8] {
        &self.0
    }
}

pub fn verify_archive(
    bytes: Vec<u8>,
    signature: &str,
    pubkey: &str,
    expected: &BundleRequirements,
) -> Result<VerifiedArchive> {
    let decode = |text: &str| -> Result<String> {
        String::from_utf8(
            base64::engine::general_purpose::STANDARD
                .decode(text.trim())
                .map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())
    };
    let key = minisign_verify::PublicKey::decode(&decode(pubkey)?).map_err(|e| e.to_string())?;
    let signature =
        minisign_verify::Signature::decode(&decode(signature)?).map_err(|e| e.to_string())?;
    key.verify(&bytes, &signature, true)
        .map_err(|e| e.to_string())?;
    verify_bundle(&bytes, expected)?;
    Ok(VerifiedArchive(bytes))
}

/// Reads the signed archive without extracting it. Reject ambiguity between the
/// metadata we inspect and the paths the updater will install.
pub fn verify_bundle(bytes: &[u8], expected: &BundleRequirements) -> Result<()> {
    #[cfg(target_os = "macos")]
    {
        let mut archive = tar::Archive::new(GzDecoder::new(bytes));
        let mut root = None;
        let mut paths = HashSet::new();
        let mut metadata = None;
        let mut executables = HashMap::new();
        let mut expanded = 0u64;
        for entry in archive.entries().map_err(|e| e.to_string())? {
            let mut entry = entry.map_err(|e| e.to_string())?;
            let path = entry.path().map_err(|e| e.to_string())?.into_owned();
            let parts: Vec<_> = path
                .components()
                .map(|component| match component {
                    Component::Normal(part) => part
                        .to_str()
                        .map(str::to_owned)
                        .ok_or("Non-UTF8 archive path"),
                    _ => Err("Unsafe archive path"),
                })
                .collect::<std::result::Result<_, _>>()?;
            let first = parts
                .first()
                .filter(|p| p.ends_with(".app"))
                .ok_or("Expected a single .app archive root")?;
            if root.as_ref().is_some_and(|r| r != first) {
                return Err("Multiple app roots".into());
            }
            root = Some(first.clone());
            if !paths.insert(path) || paths.len() > MAX_ENTRIES {
                return Err("Duplicate or excessive archive entries".into());
            }
            let kind = entry.header().entry_type();
            // This app's release format contains regular files and directories only.
            // Reject links, devices and extension entries rather than following them.
            if !kind.is_file() && !kind.is_dir() {
                return Err("Unsupported archive entry type".into());
            }
            expanded = expanded
                .checked_add(entry.size())
                .ok_or("Archive size overflow")?;
            if expanded > MAX_EXPANDED {
                return Err("Expanded archive is too large".into());
            }
            if parts.len() == 3 && parts[1] == "Contents" && parts[2] == "Info.plist" {
                if !kind.is_file() || entry.size() > MAX_METADATA {
                    return Err("Invalid bundle metadata".into());
                }
                let mut data = Vec::new();
                entry.read_to_end(&mut data).map_err(|e| e.to_string())?;
                metadata =
                    Some(plist::Value::from_reader(Cursor::new(data)).map_err(|e| e.to_string())?);
            } else if parts.len() == 4
                && parts[1] == "Contents"
                && parts[2] == "MacOS"
                && kind.is_file()
            {
                let mut header = Vec::new();
                entry
                    .take(1024)
                    .read_to_end(&mut header)
                    .map_err(|e| e.to_string())?;
                executables.insert(parts[3].clone(), header);
            }
        }
        let metadata = metadata.ok_or("Missing bundle Info.plist")?;
        validate_bundle_metadata(&metadata, expected)?;
        let executable = metadata
            .as_dictionary()
            .and_then(|m| m.get("CFBundleExecutable"))
            .and_then(plist::Value::as_string)
            .filter(|name| !name.is_empty() && !name.contains(['/', '\\']))
            .ok_or("Missing or invalid CFBundleExecutable")?;
        let header = executables
            .get(executable)
            .ok_or("Missing bundle executable")?;
        if !mach_o_has_arch(header, &expected.arch) {
            return Err("Bundle architecture does not match target".into());
        }
        Ok(())
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = bytes;
        let _ = expected;
        Ok(())
    }
}

pub fn validate_bundle_metadata(
    metadata: &plist::Value,
    expected: &BundleRequirements,
) -> Result<()> {
    let dict = metadata
        .as_dictionary()
        .ok_or("Invalid Info.plist dictionary")?;
    let get = |key| {
        dict.get(key)
            .and_then(plist::Value::as_string)
            .ok_or_else(|| format!("Missing {key}"))
    };
    let version = Version::parse(get("CFBundleShortVersionString")?).map_err(|e| e.to_string())?;
    if version != expected.version {
        return Err("Signed bundle version does not match release version".into());
    }
    let build = Version::parse(get("CFBundleVersion")?).map_err(|e| e.to_string())?;
    if build != expected.version {
        return Err("Signed bundle build version does not match release version".into());
    }
    if get("CFBundleIdentifier")? != expected.identifier {
        return Err("Signed bundle identifier does not match app".into());
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn mach_o_has_arch(bytes: &[u8], arch: &str) -> bool {
    let cpu = match arch {
        "aarch64" => 0x0100_000c,
        "x86_64" => 0x0100_0007,
        _ => return false,
    };
    if bytes.len() < 8 {
        return false;
    }
    match &bytes[..4] {
        [0xcf, 0xfa, 0xed, 0xfe] => u32::from_le_bytes(bytes[4..8].try_into().unwrap()) == cpu,
        [0xfe, 0xed, 0xfa, 0xcf] => u32::from_be_bytes(bytes[4..8].try_into().unwrap()) == cpu,
        [0xca, 0xfe, 0xba, 0xbe] | [0xca, 0xfe, 0xba, 0xbf] => {
            let count = u32::from_be_bytes(bytes[4..8].try_into().unwrap()) as usize;
            let stride = if bytes[3] == 0xbf { 32 } else { 20 };
            count <= 16
                && bytes.len() >= 8 + count * stride
                && (0..count).any(|i| {
                    u32::from_be_bytes(bytes[8 + i * stride..12 + i * stride].try_into().unwrap())
                        == cpu
                })
        }
        _ => false,
    }
}

pub fn ensure_newer(target: &Version, current: &Version) -> Result<()> {
    if !target.cmp_precedence(current).is_gt() {
        return Err("Update must be newer than the installed app".into());
    }
    Ok(())
}

fn public_key_material(encoded: &str) -> Result<Vec<u8>> {
    let text = base64::engine::general_purpose::STANDARD
        .decode(encoded.trim())
        .map_err(|e| e.to_string())?;
    let text = std::str::from_utf8(&text).map_err(|e| e.to_string())?;
    minisign_verify::PublicKey::decode(text).map_err(|e| e.to_string())?;
    let raw = base64::engine::general_purpose::STANDARD
        .decode(text.lines().nth(1).ok_or("Missing public key bytes")?)
        .map_err(|e| e.to_string())?;
    Ok(raw.get(10..).ok_or("Invalid public key")?.to_vec())
}

fn insecure_flag(plugin: &Value, names: &[&str]) -> Result<bool> {
    let mut enabled = false;
    for name in names {
        if let Some(value) = plugin.get(*name) {
            enabled |= value
                .as_bool()
                .ok_or_else(|| format!("Invalid boolean flag {name}"))?;
        }
    }
    Ok(enabled)
}

/// Only the explicit lab feature AND isolated identifier may enable HTTP.
/// Ordinary debug builds retain production trust/endpoint rules.
pub fn lab_endpoint(config: &Value, lab_enabled: bool) -> Result<Option<Url>> {
    let plugin = &config["plugins"]["updater"];
    if insecure_flag(
        plugin,
        &[
            "dangerousAcceptInvalidCerts",
            "dangerous-accept-invalid-certs",
            "dangerousAcceptInvalidHostnames",
            "dangerous-accept-invalid-hostnames",
        ],
    )? {
        return Err("Invalid TLS certificates are forbidden".into());
    }
    if lab_enabled && config["identifier"] == LAB_ID {
        let endpoints = plugin["endpoints"]
            .as_array()
            .ok_or("Missing lab endpoints")?;
        if endpoints.len() != 1 || plugin["dangerousInsecureTransportProtocol"] != true {
            return Err("Lab requires one explicit loopback endpoint".into());
        }
        let endpoint = Url::parse(endpoints[0].as_str().ok_or("Invalid lab endpoint")?)
            .map_err(|e| e.to_string())?;
        if endpoint.scheme() != "http"
            || endpoint.host_str() != Some("127.0.0.1")
            || endpoint.port().is_none()
            || endpoint.path() != "/latest.json"
            || endpoint.query().is_some()
            || endpoint.fragment().is_some()
            || !endpoint.username().is_empty()
            || endpoint.password().is_some()
        {
            return Err("Lab endpoint must be an explicit 127.0.0.1 HTTP URL".into());
        }
        let production: Value =
            serde_json::from_str(include_str!("../tauri.conf.json")).map_err(|e| e.to_string())?;
        if public_key_material(plugin["pubkey"].as_str().ok_or("Missing lab public key")?)?
            == public_key_material(
                production["plugins"]["updater"]["pubkey"]
                    .as_str()
                    .ok_or("Missing production public key")?,
            )?
        {
            return Err("Lab must use a separate signing key".into());
        }
        return Ok(Some(endpoint));
    }
    if config["identifier"] == LAB_ID
        || insecure_flag(
            plugin,
            &[
                "dangerousInsecureTransportProtocol",
                "dangerous-insecure-transport-protocol",
            ],
        )?
    {
        return Err("Lab updater settings require the lab feature and isolated identifier".into());
    }
    let endpoints = plugin["endpoints"]
        .as_array()
        .ok_or("Missing production endpoint")?;
    let expected = format!("https://github.com/{REPOSITORY}/releases/latest/download/latest.json");
    if endpoints.len() != 1 || endpoints[0] != expected {
        return Err("Unexpected production updater endpoint".into());
    }
    Ok(None)
}

pub fn validate_asset_url(url: &Url, version: &Version, lab: Option<&Url>) -> Result<String> {
    if !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err("Invalid updater asset URL".into());
    }
    let filename = if let Some(endpoint) = lab {
        if url.origin() != endpoint.origin() {
            return Err("Lab asset must stay on its loopback origin".into());
        }
        let parts: Vec<_> = url.path_segments().ok_or("Invalid asset path")?.collect();
        if parts.len() != 1 {
            return Err("Invalid lab asset path".into());
        }
        parts[0].to_owned()
    } else {
        let prefix = format!("https://github.com/{REPOSITORY}/releases/download/v{version}/");
        url.as_str()
            .strip_prefix(&prefix)
            .filter(|name| !name.contains('/'))
            .ok_or("Asset must belong to the selected release tag")?
            .to_owned()
    };
    let filename = percent_encoding::percent_decode_str(&filename)
        .decode_utf8()
        .map_err(|e| e.to_string())?
        .into_owned();
    if !filename.ends_with(".app.tar.gz")
        || filename.contains(['/', '\\'])
        || filename.starts_with('.')
    {
        return Err("Invalid updater archive filename".into());
    }
    Ok(filename)
}

pub fn verify_release(
    config: &Value,
    manifest: &Value,
    artifacts: &Path,
    target: &str,
) -> Result<()> {
    verify_manifest(config, manifest, artifacts, target, false)
}

pub fn verify_manifest(
    config: &Value,
    manifest: &Value,
    artifacts: &Path,
    target: &str,
    lab_enabled: bool,
) -> Result<()> {
    if !lab_enabled && config["identifier"] != PRODUCTION_ID {
        return Err("Release must use production identity".into());
    }
    let lab = lab_endpoint(config, lab_enabled)?;
    if lab_enabled && lab.is_none() {
        return Err("Expected isolated lab config".into());
    }
    serde_json::from_value::<tauri_plugin_updater::RemoteRelease>(manifest.clone())
        .map_err(|e| e.to_string())?;
    let version = Version::parse(
        config["version"]
            .as_str()
            .ok_or("Missing release version")?,
    )
    .map_err(|e| e.to_string())?;
    if manifest["version"].as_str() != Some(version.to_string().as_str()) {
        return Err("Manifest version does not match config".into());
    }
    let arch = target
        .strip_prefix("darwin-")
        .filter(|a| ["aarch64", "x86_64"].contains(a))
        .ok_or("Unsupported release target")?;
    let platform = &manifest["platforms"][target];
    let url = Url::parse(platform["url"].as_str().ok_or("Missing asset URL")?)
        .map_err(|e| e.to_string())?;
    let filename = validate_asset_url(&url, &version, lab.as_ref())?;
    let bytes = std::fs::read(artifacts.join(filename)).map_err(|e| e.to_string())?;
    verify_archive(
        bytes,
        platform["signature"].as_str().ok_or("Missing signature")?,
        config["plugins"]["updater"]["pubkey"]
            .as_str()
            .ok_or("Missing public key")?,
        &BundleRequirements {
            version,
            identifier: if lab_enabled {
                LAB_ID.into()
            } else {
                PRODUCTION_ID.into()
            },
            arch: arch.into(),
        },
    )?;
    Ok(())
}
