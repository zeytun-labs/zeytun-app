//! Background subscription auto-updater.
//!
//! Runs a non-blocking Tokio task that ticks once on startup and then every
//! hour. On each tick it asks the core for profiles that are due — a profile is
//! due when it has a subscription URL, is not opted out (`skip_auto_update`),
//! and `now - last_updated >= update_interval_hours`. Due profiles are refreshed
//! silently (background sync: the summary is persisted with `unread_summary`),
//! and the frontend is notified with `profile-updated` so it re-syncs.

use std::sync::Arc;
use std::time::Duration;

use tauri::{AppHandle, Emitter};
use tokio::sync::Mutex;

use crate::core::manager::CoreManager;
use crate::core::subscription::SubscriptionManager;

/// How often the updater wakes up to re-evaluate due profiles.
const TICK_INTERVAL: Duration = Duration::from_secs(60 * 60); // 1 hour

/// Spawn the auto-updater. Returns a join handle plus a shutdown sender; drop
/// the sender (or send on it) to stop the loop.
pub fn start_auto_updater(
    app: AppHandle,
    core: Arc<Mutex<CoreManager>>,
) -> (
    tauri::async_runtime::JoinHandle<()>,
    tokio::sync::oneshot::Sender<()>,
) {
    let (tx, mut rx) = tokio::sync::oneshot::channel::<()>();

    let handle = tauri::async_runtime::spawn(async move {
        // First tick shortly after startup (give the core a moment to settle),
        // then every hour.
        let mut interval = tokio::time::interval(TICK_INTERVAL);
        // `interval` fires immediately on first `tick()`, which is what we want
        // for the startup run.
        loop {
            tokio::select! {
                _ = &mut rx => {
                    println!("[AutoUpdate] shutdown signal received");
                    break;
                }
                _ = interval.tick() => {
                    run_due_updates(&app, &core).await;
                }
            }
        }
    });

    (handle, tx)
}

/// Evaluate all profiles and silently refresh the ones that are due.
async fn run_due_updates(app: &AppHandle, core: &Arc<Mutex<CoreManager>>) {
    // Snapshot the due profile ids under a short lock, then release it before
    // doing the (blocking) network refreshes.
    // We also need their URLs so we can fetch concurrently without loading from DB every time.
    let mut due_profiles = Vec::new();
    // Read the subscription route once, under the same lock: it is a DB read and
    // every profile in this batch uses the same policy.
    let sub_proxy;
    {
        let guard = core.lock().await;
        sub_proxy = guard.proxy_for_traffic(crate::core::network_policy::TRAFFIC_SUBSCRIPTION);
        match guard.profile_list() {
            Ok(profiles) => {
                for p in profiles.into_iter().filter(is_due) {
                    if let Some(url) = p.subscription_url {
                        due_profiles.push((p.id, url));
                    }
                }
            }
            Err(e) => {
                println!("[AutoUpdate] failed to list profiles: {e:?}");
                return;
            }
        }
    }

    if due_profiles.is_empty() {
        return;
    }
    println!(
        "[AutoUpdate] {} profile(s) due for update",
        due_profiles.len()
    );

    // Map over `due_profiles` to spawn blocking HTTP requests in parallel
    let mut tasks = tokio::task::JoinSet::new();
    for (id, url) in due_profiles {
        let core_arc = core.clone();
        let proxy = sub_proxy.clone();
        tasks.spawn_blocking(move || {
            println!("[AutoUpdate] fetching {id}...");
            let fetch_res = SubscriptionManager::fetch_url(&url, proxy.as_deref());

            // Take the short lock just to apply & save to the database
            let mut guard = core_arc.blocking_lock();

            match fetch_res {
                Ok((raw, headers)) => match guard.profile_apply_refresh(&id, &raw, headers, true) {
                    Ok(summary) => Ok((id, summary)),
                    Err(e) => Err(e),
                },
                Err(e) => Err(crate::error::CommandError::Network(format!(
                    "fetch failed: {e}"
                ))),
            }
        });
    }

    let mut any_updated = false;
    while let Some(res) = tasks.join_next().await {
        match res {
            Ok(Ok((id, summary))) => {
                any_updated = true;
                println!(
                    "[AutoUpdate] refreshed `{id}`: +{} -{} orphaned={}",
                    summary.proxies_added, summary.proxies_removed, summary.rules_orphaned
                );
            }
            Ok(Err(e)) => println!("[AutoUpdate] refresh failed: {e:?}"),
            Err(e) => println!("[AutoUpdate] refresh task panicked: {e:?}"),
        }
    }

    if any_updated {
        let _ = app.emit("profile-updated", ());
    }
}

/// A profile is due when it has a non-empty subscription URL, is not opted out,
/// and enough time has elapsed since its last update (or it never updated).
fn is_due(p: &crate::core::dto::ProfileMeta) -> bool {
    let has_url = p
        .subscription_url
        .as_deref()
        .map(|u| !u.trim().is_empty())
        .unwrap_or(false);
    if !has_url || p.skip_auto_update {
        return false;
    }

    let interval_ms = i64::from(p.update_interval_hours.max(1)) * 60 * 60 * 1000;
    match p.last_updated_unix_ms {
        // Never updated → due now.
        None => true,
        Some(last) => now_unix_ms().saturating_sub(last) >= interval_ms,
    }
}

fn now_unix_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or_default()
}
