//! Background GC for temporary route rules.
//!
//! Sleeps until the soonest `expires_at`, then prunes + reloads core so expired
//! matches stop applying without waiting for a manual publish.
//! Emits `profile-updated` when anything was dropped.

use std::sync::Arc;
use std::time::Duration;

use tauri::{AppHandle, Emitter};
use tokio::sync::Mutex;

use crate::core::manager::CoreManager;

const POLL_FALLBACK: Duration = Duration::from_secs(30);
const MIN_SLEEP: Duration = Duration::from_secs(1);
const MAX_SLEEP: Duration = Duration::from_secs(60 * 30);

pub fn start_temp_rule_gc(
    app: AppHandle,
    core: Arc<Mutex<CoreManager>>,
) -> (
    tauri::async_runtime::JoinHandle<()>,
    tokio::sync::oneshot::Sender<()>,
) {
    let (tx, mut rx) = tokio::sync::oneshot::channel::<()>();

    let handle = tauri::async_runtime::spawn(async move {
        loop {
            let sleep_for = next_sleep(&core).await;
            tokio::select! {
                _ = &mut rx => break,
                _ = tokio::time::sleep(sleep_for) => {
                    let core = core.clone();
                    let pruned = tauri::async_runtime::spawn_blocking(move || {
                        let mut guard = core.blocking_lock();
                        guard.prune_expired_temp_rules()
                    })
                    .await;
                    match pruned {
                        Ok(Ok(true)) => {
                            let _ = app.emit("profile-updated", ());
                        }
                        Ok(Ok(false)) => {}
                        Ok(Err(e)) => {
                            eprintln!("[TempRuleGC] prune failed: {e:?}");
                        }
                        Err(e) => {
                            eprintln!("[TempRuleGC] task panicked: {e:?}");
                        }
                    }
                }
            }
        }
    });

    (handle, tx)
}

async fn next_sleep(core: &Arc<Mutex<CoreManager>>) -> Duration {
    let core = core.clone();
    let ms = tauri::async_runtime::spawn_blocking(move || {
        let guard = core.blocking_lock();
        guard.next_temp_rule_expiry_ms()
    })
    .await
    .ok()
    .flatten();

    let Some(expires_at) = ms else {
        return POLL_FALLBACK;
    };

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0);
    let delta = expires_at.saturating_sub(now).max(0) as u64;
    let d = Duration::from_millis(delta.max(MIN_SLEEP.as_millis() as u64));
    d.min(MAX_SLEEP)
}
