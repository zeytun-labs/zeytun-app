//! GeoIP country database download.
//!
//! Source is DB-IP Country Lite (CC BY 4.0, no account, monthly). It decodes as
//! `maxminddb::geoip2::Country` with real ISO codes, so [`super::ip_info`] needs
//! no changes.
//!
//! The updated database is written to the app data directory, never into the app
//! bundle: `resources/` also holds the `setuid root` core binary, and the bundle
//! may not even be writable once installed. [`resolve_db_path`] prefers the
//! downloaded copy and falls back to the bundled one.

use std::io::Read;
use std::path::{Path, PathBuf};

/// `{}` is replaced with `YYYY-MM`.
const DBIP_URL_TEMPLATE: &str = "https://download.db-ip.com/free/dbip-country-lite-{}.mmdb.gz";

/// Metadata separator present in every valid mmdb regardless of vendor.
const MMDB_METADATA_MARKER: &[u8] = b"\xab\xcd\xefMaxMind.com";

/// `database_type` string; catches a source silently changing what it publishes.
const EXPECTED_DATABASE_TYPE: &[u8] = b"DBIP-Country-Lite";

/// Observed ~8.3 MB. A CDN error page or truncated transfer lands far below this.
const MIN_DB_BYTES: usize = 4_000_000;

/// Hard ceiling on what we will buffer, compressed or not.
const MAX_DB_BYTES: u64 = 64 * 1024 * 1024;

const DOWNLOAD_TIMEOUT_SECS: u64 = 180;

/// HEAD probes only touch metadata; keep them snappy so a startup check never
/// hangs the app if the CDN stalls.
const HEAD_TIMEOUT_SECS: u64 = 15;

#[derive(Debug, serde::Serialize)]
pub struct GeoipUpdate {
    /// `YYYY-MM` of the published file that was installed.
    pub month: String,
    /// True when the configured proxy failed and the direct route was used.
    pub degraded_to_direct: bool,
}

/// `(year, month)` in UTC, `offset_months` back. Correct across year boundaries.
fn year_month(offset_months: i32) -> (i32, u8) {
    let now = time::OffsetDateTime::now_utc();
    let total = now.year() * 12 + (u8::from(now.month()) as i32 - 1) - offset_months;
    (total.div_euclid(12), (total.rem_euclid(12) + 1) as u8)
}

/// The database to read from: the downloaded copy if present, else the bundled one.
pub fn resolve_db_path(app_data_dir: &Path, resource_dir: &Path) -> PathBuf {
    let downloaded = app_data_dir.join(crate::core::constants::GEOIP_DB_FILENAME);
    if downloaded.is_file() {
        return downloaded;
    }
    resource_dir.join(crate::core::constants::GEOIP_COUNTRY_PATH)
}

fn agent(proxy: Option<&str>) -> Result<ureq::Agent, String> {
    let mut builder =
        ureq::AgentBuilder::new().timeout(std::time::Duration::from_secs(DOWNLOAD_TIMEOUT_SECS));
    if let Some(raw) = proxy {
        // A malformed proxy is an error, not a silent fall back to direct.
        let parsed =
            ureq::Proxy::new(raw).map_err(|e| format!("invalid proxy URL `{raw}`: {e}"))?;
        builder = builder.proxy(parsed);
    }
    Ok(builder.build())
}

/// Fetch and gunzip one month's file. Returns the decompressed mmdb bytes.
fn fetch_month(agent: &ureq::Agent, month: &str) -> Result<Vec<u8>, String> {
    let url = DBIP_URL_TEMPLATE.replace("{}", month);
    let response = agent
        .get(&url)
        .call()
        .map_err(|e| format!("download failed for {month}: {e}"))?;

    let mut gz = Vec::new();
    response
        .into_reader()
        .take(MAX_DB_BYTES)
        .read_to_end(&mut gz)
        .map_err(|e| format!("read failed for {month}: {e}"))?;

    let mut raw = Vec::new();
    flate2::read::GzDecoder::new(&gz[..])
        .take(MAX_DB_BYTES)
        .read_to_end(&mut raw)
        .map_err(|e| format!("gunzip failed for {month}: {e}"))?;

    Ok(raw)
}

/// Structural validation, before the bytes go anywhere near the live path.
fn validate_bytes(raw: &[u8]) -> Result<(), String> {
    if raw.len() < MIN_DB_BYTES {
        return Err(format!("suspiciously small database: {} bytes", raw.len()));
    }
    if !contains(raw, MMDB_METADATA_MARKER) {
        return Err("not a valid mmdb: metadata marker missing".into());
    }
    if !contains(raw, EXPECTED_DATABASE_TYPE) {
        return Err("unexpected database_type: expected DBIP-Country-Lite".into());
    }
    Ok(())
}

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    haystack.windows(needle.len()).any(|w| w == needle)
}

/// `YYYY-MM` the installed database was published for, from its mmdb `build_epoch`.
///
/// DB-IP stamps the first of the month, so the timestamp identifies the release
/// without a sidecar file. `None` when nothing is installed or the metadata is
/// unreadable — callers treat both as "unknown, so update".
pub fn installed_month(db: &Path) -> Option<String> {
    let reader = maxminddb::Reader::open_readfile(db).ok()?;
    let epoch = reader.metadata.build_epoch;
    // build_epoch 0 would mean "no metadata"; treat it as unknown rather than 1970.
    if epoch == 0 {
        return None;
    }
    let dt = time::OffsetDateTime::from_unix_timestamp(epoch as i64).ok()?;
    Some(format!("{:04}-{:02}", dt.year(), u8::from(dt.month())))
}

/// Newest month DB-IP has actually published, probed with HEAD only.
///
/// The current month is not always published on the 1st, so this walks back at
/// most two months and stops at the first 200. A HEAD costs one round trip
/// instead of an 8 MB download — which is what makes a per-launch check cheap
/// enough to run unconditionally. `None` when the network or the proxy is down:
/// an unreachable CDN must not look like "an update is available".
pub fn latest_published_month(proxy: Option<&str>) -> Option<String> {
    let mut builder = ureq::AgentBuilder::new()
        // Short timeout: a hung CDN must not stall app startup.
        .timeout(std::time::Duration::from_secs(HEAD_TIMEOUT_SECS));
    if let Some(raw) = proxy {
        builder = builder.proxy(ureq::Proxy::new(raw).ok()?);
    }
    let probe = builder.build();

    for offset in 0..2 {
        let (y, m) = year_month(offset);
        let month = format!("{y:04}-{m:02}");
        let url = DBIP_URL_TEMPLATE.replace("{}", &month);
        match probe.head(&url).call() {
            Ok(_) => return Some(month),
            Err(ureq::Error::Status(404, _)) => continue, // not published yet
            Err(_) => return None,                        // network/proxy problem
        }
    }
    None
}

/// ISO country code for `ip`, or `None` when the database has no answer.
pub fn lookup_iso(db: &Path, ip: &str) -> Option<String> {
    let reader = maxminddb::Reader::open_readfile(db).ok()?;
    let addr = ip.parse::<std::net::IpAddr>().ok()?;
    let country = reader
        .lookup(addr)
        .ok()?
        .decode::<maxminddb::geoip2::Country>()
        .ok()??;
    country.country.iso_code.map(str::to_owned)
}

/// Download, validate, and atomically install the country database into `dest`.
///
/// `proxy` is the resolved route for the `geoip` traffic class. On failure with a
/// proxy set, one direct retry follows — otherwise a dead proxy would pin the
/// database at whatever version shipped.
pub fn update_country_db(dest: &Path, proxy: Option<&str>) -> Result<GeoipUpdate, String> {
    let dir = dest
        .parent()
        .ok_or_else(|| format!("destination has no parent directory: {dest:?}"))?;
    std::fs::create_dir_all(dir).map_err(|e| format!("cannot create {dir:?}: {e}"))?;

    // The current month is not always published on the 1st; fall back one month.
    let months: Vec<String> = (0..2)
        .map(|offset| {
            let (y, m) = year_month(offset);
            format!("{y:04}-{m:02}")
        })
        .collect();

    let mut degraded = false;
    let mut errors = Vec::new();
    let mut installed: Option<(String, Vec<u8>)> = None;

    'outer: for use_direct in [false, true] {
        if use_direct {
            if proxy.is_none() || installed.is_some() {
                break;
            }
            degraded = true;
        }
        let route = if use_direct { None } else { proxy };
        let agent = match agent(route) {
            Ok(a) => a,
            Err(e) => {
                errors.push(e);
                continue;
            }
        };
        for month in &months {
            match fetch_month(&agent, month).and_then(|raw| {
                validate_bytes(&raw)?;
                Ok(raw)
            }) {
                Ok(raw) => {
                    installed = Some((month.clone(), raw));
                    break 'outer;
                }
                Err(e) => errors.push(e),
            }
        }
    }

    let (month, raw) = installed.ok_or_else(|| errors.join("; "))?;
    install_verified(dest, &raw)?;

    Ok(GeoipUpdate {
        month,
        degraded_to_direct: degraded,
    })
}

/// What an automatic (background) GeoIP refresh ended up doing.
///
/// Distinguishing "already current" from "updated" lets the caller stay silent
/// on the common no-op path, and separating a failed *probe* from a failed
/// *download* means a dead CDN can't be reported as a broken update.
#[derive(Debug, serde::Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum GeoipAutoUpdate {
    /// Installed a newer month than what was on disk.
    Updated(GeoipUpdate),
    /// Already on the newest published month; nothing downloaded.
    Current { month: Option<String> },
    /// Could not reach the CDN to ask; left the installed database untouched.
    ProbeFailed,
    /// Probe saw a newer month but the download/install failed.
    UpdateFailed { reason: String },
}

/// Background refresh: HEAD-probe the newest published month, download only when
/// it is newer than what's installed.
///
/// Cheap by design — the no-op path is one HEAD round trip, not an 8 MB pull —
/// so it is safe to call on every app launch.
pub fn auto_update_country_db(
    dest: &Path,
    current_db: &Path,
    proxy: Option<&str>,
) -> GeoipAutoUpdate {
    // Match the database used by IP lookups, including the bundled fallback on
    // a fresh install. Downloads still go exclusively into app data (`dest`).
    let installed = installed_month(current_db);
    let Some(latest) = latest_published_month(proxy) else {
        return GeoipAutoUpdate::ProbeFailed;
    };

    // Already current: newest published == what's on disk.
    if installed.as_deref() == Some(latest.as_str()) {
        return GeoipAutoUpdate::Current { month: installed };
    }

    match update_country_db(dest, proxy) {
        Ok(update) => GeoipAutoUpdate::Updated(update),
        Err(reason) => GeoipAutoUpdate::UpdateFailed { reason },
    }
}

/// Stage `raw` beside `dest`, prove it answers a known lookup, then swap it in.
///
/// Never writes `dest` directly: a truncated write would leave an unreadable
/// database with no way back, in a directory that also holds the `setuid root`
/// core binary.
fn install_verified(dest: &Path, raw: &[u8]) -> Result<(), String> {
    let dir = dest
        .parent()
        .ok_or_else(|| format!("destination has no parent directory: {dest:?}"))?;
    // Same directory: rename is only atomic within one filesystem.
    let staged = dir.join(format!(
        "{}.download",
        crate::core::constants::GEOIP_DB_FILENAME
    ));
    std::fs::write(&staged, raw).map_err(|e| format!("cannot write {staged:?}: {e}"))?;

    if lookup_iso(&staged, "8.8.8.8").as_deref() != Some("US") {
        let _ = std::fs::remove_file(&staged);
        return Err("downloaded database failed a known-answer lookup (8.8.8.8 → US)".into());
    }

    std::fs::rename(&staged, dest).map_err(|e| format!("cannot install {dest:?}: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn year_month_walks_back_across_new_year() {
        let (y0, m0) = year_month(0);
        let (y1, m1) = year_month(1);
        assert!((1..=12).contains(&m1));
        if m0 == 1 {
            assert_eq!((y1, m1), (y0 - 1, 12));
        } else {
            assert_eq!((y1, m1), (y0, m0 - 1));
        }
    }

    #[test]
    fn validate_rejects_short_and_wrong_vendor() {
        assert!(validate_bytes(b"too small").is_err());

        let mut fake = vec![0u8; MIN_DB_BYTES];
        fake.extend_from_slice(MMDB_METADATA_MARKER);
        fake.extend_from_slice(b"GeoLite2-Country");
        let err = validate_bytes(&fake).unwrap_err();
        assert!(err.contains("database_type"), "got: {err}");
    }

    #[test]
    fn validate_accepts_a_wellformed_header() {
        let mut ok = vec![0u8; MIN_DB_BYTES];
        ok.extend_from_slice(MMDB_METADATA_MARKER);
        ok.extend_from_slice(EXPECTED_DATABASE_TYPE);
        assert!(validate_bytes(&ok).is_ok());
    }

    /// The real end-to-end install path, exercised against the bundled asset:
    /// stage → known-answer lookup → atomic rename. Skipped when the asset is
    /// absent so a checkout without resources still passes.
    #[test]
    fn install_verified_swaps_in_a_good_db_and_rejects_junk() {
        let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("resources/geoip.mmdb");
        if !src.exists() {
            return;
        }
        let raw = std::fs::read(&src).unwrap();

        let dir = std::env::temp_dir().join(format!("zeytun-geoip-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let dest = dir.join(crate::core::constants::GEOIP_DB_FILENAME);

        install_verified(&dest, &raw).unwrap();
        assert_eq!(lookup_iso(&dest, "8.8.8.8").as_deref(), Some("US"));

        // Junk that passes no lookup must be rejected and must not clobber the
        // database already installed.
        let junk = vec![0u8; MIN_DB_BYTES];
        assert!(install_verified(&dest, &junk).is_err());
        assert_eq!(lookup_iso(&dest, "8.8.8.8").as_deref(), Some("US"));
        assert!(!dir
            .join(format!(
                "{}.download",
                crate::core::constants::GEOIP_DB_FILENAME
            ))
            .exists());

        std::fs::remove_dir_all(&dir).ok();
    }

    /// Guards the shipped asset. Skipped when the resource is absent so a fresh
    /// checkout without assets does not fail.
    #[test]
    fn bundled_country_db_decodes() {
        let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("resources/geoip.mmdb");
        if !p.exists() {
            return;
        }
        assert_eq!(lookup_iso(&p, "8.8.8.8").as_deref(), Some("US"));
        assert_eq!(lookup_iso(&p, "5.160.1.1").as_deref(), Some("IR"));
    }

    /// Real network end-to-end: download the current DB-IP month, validate,
    /// stage, known-answer lookup, atomic install. `#[ignore]` because it needs
    /// internet and hits a public CDN — run with
    /// `cargo test --lib geoip_real -- --ignored --nocapture`.
    ///
    /// Destination is always a throwaway temp dir, never the app data dir or
    /// `resources/` (which holds the setuid-root core binary).
    #[test]
    #[ignore = "requires network: downloads the live DB-IP country database"]
    fn geoip_real_download_installs_a_working_db() {
        let dir = std::env::temp_dir().join(format!(
            "zeytun-geoip-real-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        let dest = dir.join(crate::core::constants::GEOIP_DB_FILENAME);

        let result = update_country_db(&dest, None).expect("update_country_db failed");
        println!(
            "month={} degraded_to_direct={}",
            result.month, result.degraded_to_direct
        );

        // `YYYY-MM`, and one of the two months the downloader tries.
        assert!(
            result.month.len() == 7
                && result.month.as_bytes()[4] == b'-'
                && result.month[..4].chars().all(|c| c.is_ascii_digit())
                && result.month[5..].chars().all(|c| c.is_ascii_digit()),
            "month is not YYYY-MM: {:?}",
            result.month
        );
        let (y0, m0) = year_month(0);
        let (y1, m1) = year_month(1);
        assert!(
            result.month == format!("{y0:04}-{m0:02}")
                || result.month == format!("{y1:04}-{m1:02}"),
            "month {} is neither this month ({y0:04}-{m0:02}) nor last ({y1:04}-{m1:02})",
            result.month
        );

        assert_eq!(lookup_iso(&dest, "8.8.8.8").as_deref(), Some("US"));
        assert_eq!(lookup_iso(&dest, "5.160.1.1").as_deref(), Some("IR"));

        // The staged file must be gone: the install is a rename, not a copy.
        let staged = dir.join(format!(
            "{}.download",
            crate::core::constants::GEOIP_DB_FILENAME
        ));
        assert!(!staged.exists(), "staged file left behind: {:?}", staged);

        let meta = std::fs::metadata(&dest).unwrap();
        let mtime = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let reader = maxminddb::Reader::open_readfile(&dest).unwrap();
        println!(
            "installed={:?} size={} bytes mtime={} ({} UTC) build_epoch={} database_type={:?}",
            dest,
            meta.len(),
            mtime,
            time::OffsetDateTime::from_unix_timestamp(mtime as i64)
                .map(|t| t
                    .format(&time::format_description::well_known::Rfc3339)
                    .unwrap_or_default())
                .unwrap_or_default(),
            reader.metadata.build_epoch,
            reader.metadata.database_type
        );
        assert!(meta.len() >= MIN_DB_BYTES as u64);
        assert!(reader.metadata.build_epoch > 0);

        std::fs::remove_dir_all(&dir).ok();
    }

    /// A fresh install already has a current database in its read-only bundle.
    /// The background check must not download it again into app data.
    #[test]
    #[ignore = "requires network: downloads and probes the live DB-IP database"]
    fn auto_update_keeps_current_bundled_db() {
        let dir = std::env::temp_dir().join(format!(
            "zeytun-geoip-bundled-{}-{}",
            std::process::id(),
            time::OffsetDateTime::now_utc().unix_timestamp_nanos()
        ));
        let app_data = dir.join("app-data");
        let resource_dir = dir.join("bundle");
        let bundled = resource_dir.join(crate::core::constants::GEOIP_COUNTRY_PATH);
        let dest = app_data.join(crate::core::constants::GEOIP_DB_FILENAME);
        let fixture = update_country_db(&bundled, None).expect("download bundle fixture");
        assert_eq!(
            latest_published_month(None).as_deref(),
            Some(fixture.month.as_str())
        );
        assert_eq!(resolve_db_path(&app_data, &resource_dir), bundled);
        let before = std::fs::read(&bundled).unwrap();

        let current_db = resolve_db_path(&app_data, &resource_dir);
        let result = auto_update_country_db(&dest, &current_db, None);
        assert!(
            matches!(result, GeoipAutoUpdate::Current { ref month } if month.as_deref() == Some(fixture.month.as_str())),
            "current bundled database must not download again: {result:?}"
        );
        assert!(!dest.exists(), "no downloaded copy should be created");
        assert_eq!(std::fs::read(&bundled).unwrap(), before);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// Proves the *auto* path: first launch (empty dest) downloads, a second
    /// launch sees the same month and no-ops. This is the wiring behind the
    /// settings toggle, exercised end to end against the live CDN.
    #[test]
    #[ignore = "requires network: probes and downloads the live DB-IP database"]
    fn auto_update_downloads_then_noops() {
        let dir = std::env::temp_dir().join(format!(
            "zeytun-geoip-auto-{}-{}",
            std::process::id(),
            time::OffsetDateTime::now_utc().unix_timestamp_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let dest = dir.join(crate::core::constants::GEOIP_DB_FILENAME);

        // No database installed yet → the month is unknown, so a probe can't
        // claim "current" and must fall through to a real download.
        assert_eq!(installed_month(&dest), None);
        match auto_update_country_db(&dest, &dest, None) {
            GeoipAutoUpdate::Updated(u) => {
                println!("first launch: downloaded {}", u.month);
                assert_eq!(lookup_iso(&dest, "8.8.8.8").as_deref(), Some("US"));
                assert_eq!(lookup_iso(&dest, "5.160.1.1").as_deref(), Some("IR"));
            }
            other => panic!("expected Updated on an empty dest, got {other:?}"),
        }

        let first = installed_month(&dest).expect("installed db has a month");
        // Second launch: same month is published, so nothing downloads.
        match auto_update_country_db(&dest, &dest, None) {
            GeoipAutoUpdate::Current { month } => {
                println!("second launch: already current ({month:?})");
                assert_eq!(month.as_deref(), Some(first.as_str()));
            }
            other => panic!("expected Current on the second launch, got {other:?}"),
        }

        std::fs::remove_dir_all(&dir).ok();
    }
}
