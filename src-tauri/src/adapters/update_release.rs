use crate::error::CommandError;
use semver::Version;
use serde_json::{json, Value};
use std::{io::Read, time::Duration};
use tauri::Manager;
use tauri_plugin_updater::UpdaterExt;
use url::Url;

const RELEASES_API: &str = "https://api.github.com/repos/zeytun-labs/zeytun-release/releases";
const RELEASE_DOWNLOAD: &str = "https://github.com/zeytun-labs/zeytun-release/releases/download/";

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

#[tauri::command]
pub async fn check_release_update(
    webview: tauri::Webview,
    proxy: Option<String>,
    include_prerelease: bool,
) -> Result<Option<Value>, CommandError> {
    let lookup_proxy = proxy.clone();
    let Some((endpoint, selected_version)) = tauri::async_runtime::spawn_blocking(move || {
        release_endpoint(lookup_proxy.as_deref(), include_prerelease)
    })
    .await
    .map_err(|e| CommandError::Internal(e.to_string()))??
    else {
        return Ok(None);
    };
    if selected_version <= webview.package_info().version {
        return Ok(None);
    }
    let mut builder = webview
        .updater_builder()
        .endpoints(vec![endpoint])
        .map_err(|e| CommandError::Internal(e.to_string()))?
        .timeout(Duration::from_secs(30));
    if let Some(proxy) = proxy {
        builder = builder.proxy(
            Url::parse(&proxy)
                .map_err(|e| CommandError::InvalidInput(format!("Invalid update proxy: {e}")))?,
        );
    }
    let updater = builder
        .build()
        .map_err(|e| CommandError::Internal(e.to_string()))?;
    let update = updater
        .check()
        .await
        .map_err(|e| CommandError::Network(e.to_string()))?;
    if update
        .as_ref()
        .is_some_and(|update| update.version != selected_version.to_string())
    {
        return Err(CommandError::Network(
            "Release manifest version does not match tag".into(),
        ));
    }
    Ok(update.map(|update| {
        let mut metadata = json!({
            "currentVersion": &update.current_version,
            "version": &update.version,
            "date": update.date.as_ref().and_then(|date| date.format(&time::format_description::well_known::Rfc3339).ok()),
            "body": &update.body,
            "rawJson": &update.raw_json,
        });
        metadata["rid"] = json!(webview.resources_table().add(update));
        metadata
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

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
