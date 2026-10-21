#![cfg(target_os = "macos")]

use base64::Engine;
use semver::Version;
use serde_json::{json, Value};
use std::path::PathBuf;
use zeytun_lib::update_security::*;

const KEY: &str = include_str!("fixtures/updater/fixture.pub");
const ARCHIVE: &[u8] = include_bytes!("fixtures/updater/release.app.tar.gz");
const SIG: &str = include_str!("fixtures/updater/release.app.tar.gz.sig");
const OLD: &[u8] = include_bytes!("fixtures/updater/older.app.tar.gz");
const OLD_SIG: &str = include_str!("fixtures/updater/older.app.tar.gz.sig");

fn expected() -> BundleRequirements {
    BundleRequirements {
        version: Version::parse("3.0.0").unwrap(),
        identifier: PRODUCTION_ID.into(),
        arch: "aarch64".into(),
    }
}
fn config() -> Value {
    let mut config: Value = serde_json::from_str(include_str!("../tauri.conf.json")).unwrap();
    config["version"] = json!("3.0.0");
    config["plugins"]["updater"]["pubkey"] = json!(KEY.trim());
    config
}
fn lab_config() -> Value {
    let mut config = config();
    config["identifier"] = json!(LAB_ID);
    config["plugins"]["updater"]["endpoints"] = json!(["http://127.0.0.1:9876/latest.json"]);
    config["plugins"]["updater"]["dangerousInsecureTransportProtocol"] = json!(true);
    config
}
fn manifest(signature: &str) -> Value {
    json!({"version":"3.0.0","platforms":{"darwin-aarch64":{
        "url":"https://github.com/zeytun-labs/zeytun-app/releases/download/v3.0.0/release.app.tar.gz",
        "signature":signature.trim()}}})
}
struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("updater-security-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn signatures_fail_closed_for_tampering_wrong_key_missing_and_malformed_data() {
    assert!(verify_archive(ARCHIVE.to_vec(), SIG, KEY, &expected()).is_ok());
    let mut tampered = ARCHIVE.to_vec();
    tampered[20] ^= 1;
    assert!(verify_archive(tampered, SIG, KEY, &expected()).is_err());
    let production: Value = serde_json::from_str(include_str!("../tauri.conf.json")).unwrap();
    assert!(verify_archive(
        ARCHIVE.to_vec(),
        SIG,
        production["plugins"]["updater"]["pubkey"].as_str().unwrap(),
        &expected()
    )
    .is_err());
    for signature in ["", "not-even-base64"] {
        assert!(verify_archive(ARCHIVE.to_vec(), signature, KEY, &expected()).is_err());
    }
}

#[test]
fn authentic_old_archive_cannot_be_relabelled_as_a_new_version() {
    assert!(verify_archive(OLD.to_vec(), OLD_SIG, KEY, &expected())
        .err()
        .unwrap()
        .contains("version"));
}

#[test]
fn signed_identity_and_architecture_must_match_the_running_app() {
    let mut target = expected();
    target.identifier = LAB_ID.into();
    assert!(verify_archive(ARCHIVE.to_vec(), SIG, KEY, &target).is_err());
    target = expected();
    target.arch = "x86_64".into();
    assert!(verify_archive(ARCHIVE.to_vec(), SIG, KEY, &target).is_err());
}

#[test]
fn strict_precedence_rejects_old_equal_and_build_only_version_changes() {
    let current = Version::parse("2.0.0").unwrap();
    for version in ["1.0.0", "2.0.0", "2.0.0+new-build", "2.0.0-alpha"] {
        assert!(ensure_newer(&Version::parse(version).unwrap(), &current).is_err());
    }
    assert!(ensure_newer(&Version::parse("2.0.1").unwrap(), &current).is_ok());
}

#[test]
fn release_gate_authenticates_archived_bytes_and_embedded_version() {
    let dir = Temp::new();
    std::fs::write(dir.0.join("release.app.tar.gz"), ARCHIVE).unwrap();
    assert!(verify_release(&config(), &manifest(SIG), &dir.0, "darwin-aarch64").is_ok());
    assert!(verify_release(
        &config(),
        &manifest("nonempty-invalid-signature"),
        &dir.0,
        "darwin-aarch64"
    )
    .is_err());
    std::fs::write(dir.0.join("release.app.tar.gz"), OLD).unwrap();
    assert!(
        verify_release(&config(), &manifest(OLD_SIG), &dir.0, "darwin-aarch64")
            .err()
            .unwrap()
            .contains("version")
    );
}

#[test]
fn release_gate_rejects_manifest_mismatches_foreign_urls_and_missing_assets() {
    let dir = Temp::new();
    let mut value = manifest(SIG);
    value["version"] = json!("4.0.0");
    assert!(verify_release(&config(), &value, &dir.0, "darwin-aarch64").is_err());
    value = manifest(SIG);
    value["platforms"]["darwin-aarch64"]["url"] =
        json!("https://example.invalid/release.app.tar.gz");
    assert!(verify_release(&config(), &value, &dir.0, "darwin-aarch64").is_err());
    assert!(verify_release(&config(), &manifest(SIG), &dir.0, "darwin-aarch64").is_err());
    let mut wrong_key = config();
    wrong_key["plugins"]["updater"]["pubkey"] =
        serde_json::from_str::<Value>(include_str!("../tauri.conf.json")).unwrap()["plugins"]
            ["updater"]["pubkey"]
            .clone();
    std::fs::write(dir.0.join("release.app.tar.gz"), ARCHIVE).unwrap();
    assert!(verify_release(&wrong_key, &manifest(SIG), &dir.0, "darwin-aarch64").is_err());
}

#[test]
fn lab_requires_explicit_feature_isolated_identity_and_loopback() {
    assert!(lab_endpoint(&config(), false).unwrap().is_none());
    assert!(lab_endpoint(&lab_config(), false).is_err());
    let lab = lab_endpoint(&lab_config(), true).unwrap().unwrap();
    assert_eq!(lab.port(), Some(9876));
    for endpoint in [
        "http://localhost:9876/latest.json",
        "http://example.invalid:9876/latest.json",
        "https://127.0.0.1:9876/latest.json",
        "http://127.0.0.1:9876/other.json",
    ] {
        let mut config = lab_config();
        config["plugins"]["updater"]["endpoints"] = json!([endpoint]);
        assert!(lab_endpoint(&config, true).is_err());
    }
    let mut impersonated = lab_config();
    impersonated["identifier"] = json!(PRODUCTION_ID);
    assert!(lab_endpoint(&impersonated, true).is_err());
}

#[test]
fn lab_cannot_reuse_production_trust_key_even_with_a_different_comment() {
    let production: Value = serde_json::from_str(include_str!("../tauri.conf.json")).unwrap();
    let encoded = production["plugins"]["updater"]["pubkey"].as_str().unwrap();
    let decoded = String::from_utf8(
        base64::engine::general_purpose::STANDARD
            .decode(encoded)
            .unwrap(),
    )
    .unwrap();
    let changed = format!(
        "untrusted comment: lab key\n{}\n",
        decoded.lines().nth(1).unwrap()
    );
    let mut config = lab_config();
    config["plugins"]["updater"]["pubkey"] =
        json!(base64::engine::general_purpose::STANDARD.encode(changed));
    assert!(lab_endpoint(&config, true).is_err());
}

#[test]
fn production_rejects_insecure_or_lab_configuration() {
    for flag in [
        "dangerousInsecureTransportProtocol",
        "dangerousAcceptInvalidCerts",
        "dangerousAcceptInvalidHostnames",
        "dangerous-insecure-transport-protocol",
        "dangerous-accept-invalid-certs",
        "dangerous-accept-invalid-hostnames",
    ] {
        let mut config = config();
        config["plugins"]["updater"][flag] = json!(true);
        assert!(lab_endpoint(&config, false).is_err());
    }
    let mut config = config();
    config["identifier"] = json!(LAB_ID);
    assert!(lab_endpoint(&config, false).is_err());
}

#[test]
fn asset_url_is_bound_to_selected_tag_and_exact_lab_origin() {
    let version = Version::parse("3.0.0").unwrap();
    for url in [
        "http://github.com/zeytun-labs/zeytun-app/releases/download/v3.0.0/a.app.tar.gz",
        "https://github.com/zeytun-labs/zeytun-app/releases/download/v1.0.0/a.app.tar.gz",
        "https://github.com/zeytun-labs/zeytun-app/releases/download/v3.0.0/%2Foutside.app.tar.gz",
        "https://github.com/zeytun-labs/zeytun-app/releases/download/v3.0.0/a.app.tar.gz?ignored",
    ] {
        assert!(validate_asset_url(&url.parse().unwrap(), &version, None).is_err());
    }
    let endpoint = lab_endpoint(&lab_config(), true).unwrap().unwrap();
    assert!(validate_asset_url(
        &"http://127.0.0.1:9876/Lab.app.tar.gz".parse().unwrap(),
        &version,
        Some(&endpoint)
    )
    .is_ok());
    assert!(validate_asset_url(
        &"http://127.0.0.1:8787/Lab.app.tar.gz".parse().unwrap(),
        &version,
        Some(&endpoint)
    )
    .is_err());
}

#[test]
fn actual_lab_archive_passes_its_isolated_policy_but_not_release_policy() {
    let dir = Temp::new();
    std::fs::write(
        dir.0.join("Lab.app.tar.gz"),
        include_bytes!("fixtures/updater/lab.app.tar.gz"),
    )
    .unwrap();
    let mut manifest = manifest(include_str!("fixtures/updater/lab.app.tar.gz.sig"));
    manifest["platforms"]["darwin-aarch64"]["url"] = json!("http://127.0.0.1:9876/Lab.app.tar.gz");
    assert!(verify_manifest(&lab_config(), &manifest, &dir.0, "darwin-aarch64", true).is_ok());
    assert!(verify_release(&lab_config(), &manifest, &dir.0, "darwin-aarch64").is_err());
}

fn archive(entries: &[(&str, u8, &[u8])]) -> Vec<u8> {
    let gzip = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    let mut tar = tar::Builder::new(gzip);
    for (path, kind, bytes) in entries {
        let mut header = tar::Header::new_gnu();
        header.set_size(bytes.len() as u64);
        header.set_mode(0o755);
        header.set_entry_type(tar::EntryType::new(*kind));
        header.set_cksum();
        tar.append_data(&mut header, path, *bytes).unwrap();
    }
    tar.into_inner().unwrap().finish().unwrap()
}

#[test]
fn duplicate_metadata_multiple_roots_links_and_missing_metadata_are_rejected() {
    let mut metadata = plist::Dictionary::new();
    for (key, value) in [
        ("CFBundleIdentifier", PRODUCTION_ID),
        ("CFBundleShortVersionString", "3.0.0"),
        ("CFBundleVersion", "3.0.0"),
        ("CFBundleExecutable", "fixture"),
    ] {
        metadata.insert(key.into(), plist::Value::String(value.into()));
    }
    let mut xml = Vec::new();
    plist::Value::Dictionary(metadata)
        .to_writer_xml(&mut xml)
        .unwrap();
    let fake = xml.as_slice();
    for bytes in [
        archive(&[
            ("A.app/Contents/Info.plist", b'0', fake),
            ("A.app/Contents/Info.plist", b'0', fake),
        ]),
        archive(&[("A.app/file", b'0', fake), ("B.app/file", b'0', fake)]),
        archive(&[("A.app/link", b'2', b"")]),
        archive(&[("A.app/file", b'0', fake)]),
    ] {
        assert!(verify_bundle(&bytes, &expected()).is_err());
    }
}

#[cfg(feature = "updater-tools")]
#[test]
fn ci_executable_uses_fail_closed_shared_verifier() {
    let dir = Temp::new();
    std::fs::write(dir.0.join("release.app.tar.gz"), ARCHIVE).unwrap();
    std::fs::write(
        dir.0.join("config.json"),
        serde_json::to_vec(&config()).unwrap(),
    )
    .unwrap();
    for (signature, success) in [(SIG, true), ("invalid", false)] {
        std::fs::write(
            dir.0.join("manifest.json"),
            serde_json::to_vec(&manifest(signature)).unwrap(),
        )
        .unwrap();
        let output = std::process::Command::new(env!("CARGO_BIN_EXE_verify-update"))
            .args([
                "--config",
                dir.0.join("config.json").to_str().unwrap(),
                "--manifest",
                dir.0.join("manifest.json").to_str().unwrap(),
                "--artifacts",
                dir.0.to_str().unwrap(),
                "--target",
                "darwin-aarch64",
            ])
            .output()
            .unwrap();
        assert_eq!(
            output.status.success(),
            success,
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

#[test]
fn bundle_build_version_and_metadata_dictionary_are_checked() {
    let mut metadata = plist::Dictionary::new();
    metadata.insert(
        "CFBundleShortVersionString".into(),
        plist::Value::String("3.0.0".into()),
    );
    metadata.insert(
        "CFBundleVersion".into(),
        plist::Value::String("1.0.0".into()),
    );
    metadata.insert(
        "CFBundleIdentifier".into(),
        plist::Value::String(PRODUCTION_ID.into()),
    );
    assert!(validate_bundle_metadata(&plist::Value::Dictionary(metadata), &expected()).is_err());
    assert!(validate_bundle_metadata(
        &plist::Value::String("not a dictionary".into()),
        &expected()
    )
    .is_err());
}
