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
use crate::core::subscription_refresh::RefreshWorkers;

/// How often the updater wakes up to re-evaluate due profiles.
const TICK_INTERVAL: Duration = Duration::from_secs(30);

/// Spawn the auto-updater. Returns a join handle plus a shutdown sender; drop
/// the sender (or send on it) to stop the loop.
pub fn start_auto_updater(
    app: AppHandle,
    core: Arc<Mutex<CoreManager>>,
    workers: Arc<RefreshWorkers>,
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
                    let cancel=Arc::new(std::sync::atomic::AtomicBool::new(false));
                    let run=run_due_updates(&app, &core, &workers, cancel.clone());
                    tokio::pin!(run);
                    tokio::select! {
                        _ = &mut run => {},
                        _ = &mut rx => {
                            cancel.store(true,std::sync::atomic::Ordering::SeqCst);
                            workers.shutdown().await;
                            run.await;
                            break;
                        }
                    }
                }
            }
        }
    });

    (handle, tx)
}

/// Evaluate all profiles and silently refresh the ones that are due.
async fn run_due_updates(
    app: &AppHandle,
    core: &Arc<Mutex<CoreManager>>,
    workers: &Arc<RefreshWorkers>,
    cancel: Arc<std::sync::atomic::AtomicBool>,
) {
    let core = core.clone();
    let sources = tokio::task::spawn_blocking(move || {
        let manager = core.blocking_lock();
        manager
            .profile_list()?
            .into_iter()
            .map(|p| Ok((p.id.clone(), manager.subscription_list(&p.id)?)))
            .collect::<Result<Vec<_>, crate::error::CommandError>>()
    })
    .await;
    let Ok(Ok(sources)) = sources else {
        return;
    };
    let now = crate::core::constants::unix_millis();
    let mut jobs = tokio::task::JoinSet::new();
    for (profile_id, sources) in sources {
        let ids = sources
            .iter()
            .filter(|s| workers.is_due_at(&profile_id, s, now))
            .map(|s| s.id.clone())
            .collect::<Vec<_>>();
        if ids.is_empty() {
            continue;
        }
        let w = workers.clone();
        let c = cancel.clone();
        jobs.spawn(async move { w.refresh_all(profile_id, ids, true, c).await });
    }
    while let Some(Ok(results)) = jobs.join_next().await {
        for result in results {
            let _ = app.emit("subscription-updated", &result);
        }
        let _ = app.emit("profile-updated", ());
    }
}
