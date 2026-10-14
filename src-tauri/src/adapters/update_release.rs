use crate::error::CommandError;
use crate::update_security::{self, BundleRequirements, VerifiedArchive};
use semver::Version;
use serde_json::{json, Value};
use std::sync::Arc;
use std::{io::Read, time::Duration};
use tauri::{ipc::Channel, Manager, Resource, ResourceId};
use tauri_plugin_updater::UpdaterExt;
use tokio::sync::Mutex;
use url::Url;

const RELEASES_API: &str = "https://api.github.com/repos/zeytun-labs/zeytun-app/releases";
const RELEASE_DOWNLOAD: &str = "https://github.com/zeytun-labs/zeytun-app/releases/download";

fn newest_release(releases: &[Value], include_prerelease: bool) -> Option<Version> {
    releases
        .iter()
        .filter(|release| release["draft"] == false)
        .filter(|release| include_prerelease || release["prerelease"] != true)
        .filter(|release| {
            release["assets"]
                .as_array()
                .is_some_and(|assets| assets.iter().any(|asset| asset["name"] == "latest.json"))
        })
        .filter_map(|release| {
            release["tag_name"]
                .as_str()
                .and_then(|tag| Version::parse(tag.strip_prefix('v').unwrap_or(tag)).ok())
        })
        .filter(|version| include_prerelease || version.pre.is_empty())
        .max()
}

fn release_endpoint(
    proxy: Option<&str>,
    include_prerelease: bool,
) -> Result<Option<(Url, Version)>, CommandError> {
    let mut agent = ureq::AgentBuilder::new().timeout(Duration::from_secs(15));
    if let Some(proxy) = proxy {
        agent = agent.proxy(
            ureq::Proxy::new(proxy)
                .map_err(|e| CommandError::InvalidInput(format!("Invalid update proxy: {e}")))?,
        );
    }
    let agent = agent.build();
    let mut newest = None;
    let mut page = 1;
    loop {
        let url = format!("{RELEASES_API}?per_page=100&page={page}");
        let response = agent
            .get(&url)
            .set("User-Agent", "Zeytun-Updater")
            .set("Accept", "application/vnd.github+json")
            .call()
            .map_err(|e| CommandError::Network(format!("Release list: {e}")))?;
        // Bound each untrusted API response, without capping the number of pages.
        let mut body = Vec::new();
        response
            .into_reader()
            .take(2 * 1024 * 1024 + 1)
            .read_to_end(&mut body)
            .map_err(|e| CommandError::Network(format!("Release list: {e}")))?;
        if body.len() > 2 * 1024 * 1024 {
            return Err(CommandError::Network("Release list too large".into()));
        }
        let releases: Vec<Value> = serde_json::from_slice(&body)
            .map_err(|e| CommandError::Network(format!("Invalid release list: {e}")))?;
        if let Some(version) = newest_release(&releases, include_prerelease) {
            newest = Some(newest.map_or(version.clone(), |current: Version| current.max(version)));
        }
        if releases.len() < 100 {
            break;
        }
        page += 1;
    }
    let Some(version) = newest else {
        return Ok(None);
    };
    let mut endpoint =
        Url::parse(RELEASE_DOWNLOAD).map_err(|e| CommandError::Internal(e.to_string()))?;
    endpoint
        .path_segments_mut()
        .map_err(|_| CommandError::Internal("Invalid release URL".into()))?
        .push(&format!("v{version}"))
        .push("latest.json");
    Ok(Some((endpoint, version)))
}

struct CheckedRelease {
    update: tauri_plugin_updater::Update,
    expected: BundleRequirements,
    pubkey: String,
    downloaded: Mutex<Option<Arc<VerifiedArchive>>>,
}
impl Resource for CheckedRelease {}

fn main_window(webview: &tauri::Webview) -> Result<(), CommandError> {
    if webview.label() != "main" {
        return Err(CommandError::InvalidInput(
            "Updater is restricted to the main window".into(),
        ));
    }
    Ok(())
}

#[tauri::command]
pub async fn check_release_update(
    webview: tauri::Webview,
    proxy: Option<String>,
    include_prerelease: bool,
) -> Result<Option<Value>, CommandError> {
    main_window(&webview)?;
    let config = serde_json::to_value(webview.config())
        .map_err(|e| CommandError::Internal(e.to_string()))?;
    let lab = update_security::lab_endpoint(&config, cfg!(feature = "updater-lab"))
        .map_err(CommandError::InvalidInput)?;
    let (endpoint, selected_version) = if let Some(endpoint) = &lab {
        (endpoint.clone(), None)
    } else {
        let lookup_proxy = proxy.clone();
        let Some((endpoint, version)) = tauri::async_runtime::spawn_blocking(move || {
            release_endpoint(lookup_proxy.as_deref(), include_prerelease)
        })
        .await
        .map_err(|e| CommandError::Internal(e.to_string()))??
        else {
            return Ok(None);
        };
        if version <= webview.package_info().version {
            return Ok(None);
        }
        (endpoint, Some(version))
    };
    let mut builder = webview
        .updater_builder()
        .endpoints(vec![endpoint])
        .map_err(|e| CommandError::Internal(e.to_string()))?
        .version_comparator(|current, remote| remote.version.cmp_precedence(&current).is_gt())
        .timeout(Duration::from_secs(30));
    // Lab traffic never leaves loopback through a configured/system proxy.
    if lab.is_some() {
        builder = builder.no_proxy();
    } else if let Some(proxy) = proxy {
        builder = builder
            .proxy(Url::parse(&proxy).map_err(|e| CommandError::InvalidInput(e.to_string()))?);
    }
    let Some(mut update) = builder
        .build()
        .map_err(|e| CommandError::Internal(e.to_string()))?
        .check()
        .await
        .map_err(|e| CommandError::Network(e.to_string()))?
    else {
        return Ok(None);
    };
    let version =
        Version::parse(&update.version).map_err(|e| CommandError::Network(e.to_string()))?;
    update_security::ensure_newer(&version, &webview.package_info().version)
        .map_err(CommandError::InvalidInput)?;
    if selected_version.is_some_and(|selected| selected != version) {
        return Err(CommandError::Network(
            "Release manifest version does not match tag".into(),
        ));
    }
    update_security::validate_asset_url(&update.download_url, &version, lab.as_ref())
        .map_err(CommandError::Network)?;
    update.timeout = Some(Duration::from_secs(120));
    let mut metadata = json!({
        "currentVersion": update.current_version,
        "version": update.version,
        "date": update.date.as_ref().and_then(|date| date.format(&time::format_description::well_known::Rfc3339).ok()),
        "body": update.body,
    });
    let checked = CheckedRelease {
        expected: BundleRequirements {
            version,
            identifier: webview.config().identifier.clone(),
            arch: std::env::consts::ARCH.into(),
        },
        pubkey: config["plugins"]["updater"]["pubkey"]
            .as_str()
            .ok_or_else(|| CommandError::Internal("Missing updater public key".into()))?
            .into(),
        update,
        downloaded: Mutex::new(None),
    };
    metadata["rid"] = json!(webview.resources_table().add(checked));
    Ok(Some(metadata))
}

#[derive(Clone, serde::Serialize)]
#[serde(tag = "event", content = "data")]
pub enum DownloadEvent {
    Started {
        #[serde(rename = "contentLength")]
        content_length: Option<u64>,
    },
    Progress {
        #[serde(rename = "chunkLength")]
        chunk_length: usize,
    },
}

#[tauri::command]
pub async fn download_release_update(
    webview: tauri::Webview,
    rid: ResourceId,
    on_event: Channel<DownloadEvent>,
) -> Result<(), CommandError> {
    main_window(&webview)?;
    let checked = webview
        .resources_table()
        .get::<CheckedRelease>(rid)
        .map_err(|e| CommandError::InvalidInput(e.to_string()))?;
    let mut downloaded = checked.downloaded.lock().await;
    if downloaded.is_some() {
        return Ok(());
    }
    let mut started = false;
    let bytes = checked
        .update
        .download(
            |chunk_length, content_length| {
                if !started {
                    started = true;
                    let _ = on_event.send(DownloadEvent::Started { content_length });
                }
                let _ = on_event.send(DownloadEvent::Progress { chunk_length });
            },
            || {},
        )
        .await
        .map_err(|e| CommandError::Network(e.to_string()))?;
    let signature = checked.update.signature.clone();
    let pubkey = checked.pubkey.clone();
    let expected = checked.expected.clone();
    let verified = tauri::async_runtime::spawn_blocking(move || {
        update_security::verify_archive(bytes, &signature, &pubkey, &expected)
    })
    .await
    .map_err(|e| CommandError::Internal(e.to_string()))?
    .map_err(CommandError::InvalidInput)?;
    *downloaded = Some(Arc::new(verified));
    Ok(())
}

static INSTALL_LOCK: Mutex<()> = Mutex::const_new(());

#[tauri::command]
pub async fn install_release_update(
    webview: tauri::Webview,
    rid: ResourceId,
) -> Result<(), CommandError> {
    main_window(&webview)?;
    let _install = INSTALL_LOCK.lock().await;
    let checked = webview
        .resources_table()
        .get::<CheckedRelease>(rid)
        .map_err(|e| CommandError::InvalidInput(e.to_string()))?;
    let mut downloaded = checked.downloaded.lock().await;
    let bytes = downloaded
        .as_ref()
        .ok_or_else(|| {
            CommandError::InvalidInput("Download and verification required before install".into())
        })?
        .clone();
    let update = checked.update.clone();
    let expected = checked.expected.clone();
    tauri::async_runtime::spawn_blocking(move || {
        // Recheck the app on disk, too: another update may have completed since
        // this resource was created. Test/dev executables cannot install apps.
        let executable = tauri::utils::platform::current_exe().map_err(|e| e.to_string())?;
        let app = tauri_plugin_updater::extract_path_from_executable(&executable)
            .map_err(|e| e.to_string())?;
        if app.extension().is_none_or(|ext| ext != "app") {
            return Err("A bundled .app is required for installation".to_owned());
        }
        let metadata =
            plist::Value::from_file(app.join("Contents/Info.plist")).map_err(|e| e.to_string())?;
        let dict = metadata
            .as_dictionary()
            .ok_or("Invalid installed bundle metadata")?;
        if dict
            .get("CFBundleIdentifier")
            .and_then(plist::Value::as_string)
            != Some(expected.identifier.as_str())
        {
            return Err("Installed bundle identity changed".into());
        }
        let current = dict
            .get("CFBundleShortVersionString")
            .and_then(plist::Value::as_string)
            .ok_or("Missing installed version")?;
        update_security::ensure_newer(
            &expected.version,
            &Version::parse(current).map_err(|e| e.to_string())?,
        )?;
        update.install(bytes.bytes()).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| CommandError::Internal(e.to_string()))?
    .map_err(CommandError::InvalidInput)?;
    *downloaded = None;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn release_manifest_url_has_single_path_separator() {
        let mut endpoint = Url::parse(RELEASE_DOWNLOAD).unwrap();
        endpoint
            .path_segments_mut()
            .unwrap()
            .push("v0.2.0-alpha")
            .push("latest.json");
        assert_eq!(
            endpoint.as_str(),
            "https://github.com/zeytun-labs/zeytun-app/releases/download/v0.2.0-alpha/latest.json"
        );
    }

    #[test]
    fn stable_channel_ignores_prereleases() {
        let releases = json!([
            {"tag_name":"v0.1.0-alpha", "draft":false,"prerelease":true,"assets":[{"name":"latest.json"}]}
        ]);
        assert_eq!(newest_release(releases.as_array().unwrap(), false), None);
    }

    #[test]
    fn stable_channel_ignores_alpha_tag_even_without_prerelease_flag() {
        let releases = json!([
            {"tag_name":"v0.2.0-alpha", "draft":false,"prerelease":false,"assets":[{"name":"latest.json"}]},
            {"tag_name":"v0.1.0", "draft":false,"prerelease":false,"assets":[{"name":"latest.json"}]}
        ]);
        assert_eq!(
            newest_release(releases.as_array().unwrap(), false),
            Some(Version::parse("0.1.0").unwrap())
        );
    }

    #[test]
    fn selects_highest_published_semver_with_manifest() {
        let releases = json!([
            {"tag_name":"v0.1.0-alpha", "draft":false,"assets":[{"name":"latest.json"}]},
            {"tag_name":"v0.1.0", "draft":false,"assets":[{"name":"latest.json"}]},
            {"tag_name":"v0.2.0-alpha", "draft":true,"assets":[{"name":"latest.json"}]},
            {"tag_name":"v0.3.0", "draft":false,"assets":[]},
            {"tag_name":"../invalid", "draft":false,"assets":[{"name":"latest.json"}]}
        ]);
        assert_eq!(
            newest_release(releases.as_array().unwrap(), true)
                .unwrap()
                .to_string(),
            "0.1.0"
        );
        let prereleases = json!([
            {"tag_name":"v0.1.0-alpha", "draft":false,"assets":[{"name":"latest.json"}]},
            {"tag_name":"v0.1.0-beta", "draft":false,"assets":[{"name":"latest.json"}]}
        ]);
        assert_eq!(
            newest_release(prereleases.as_array().unwrap(), true).unwrap(),
            Version::parse("0.1.0-beta").unwrap()
        );
    }
}
