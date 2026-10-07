//! Background task monitoring system sleep/wake cycles.
//!
//! Detects sleep transitions by monitoring elapsed time between consecutive
//! 2-second Tokio sleep ticks. If elapsed wall time exceeds 5 seconds, the system
//! was suspended. On wake, it gives network adapters 1.5s to re-associate,
//! reapplies system proxy settings on active services, and emits `system-wake`.

use std::sync::Arc;
use std::time::Duration;
use tauri::{AppHandle, Emitter};
use tokio::sync::Mutex;

use crate::core::manager::CoreManager;

pub fn start_wake_monitor(
    app: AppHandle,
    core: Arc<Mutex<CoreManager>>,
) -> (
    tauri::async_runtime::JoinHandle<()>,
    tokio::sync::oneshot::Sender<()>,
) {
    let (tx, mut rx) = tokio::sync::oneshot::channel::<()>();

    let handle = tauri::async_runtime::spawn(async move {
        let mut last_tick = std::time::Instant::now();
        loop {
            tokio::select! {
                _ = &mut rx => {
                    break;
                }
                _ = tokio::time::sleep(Duration::from_secs(2)) => {
                    let elapsed = last_tick.elapsed();
                    last_tick = std::time::Instant::now();

                    // If more than 5s elapsed during a 2s sleep, the OS was suspended
                    if elapsed >= Duration::from_secs(5) {
                        println!(
                            "[WakeMonitor] System wake detected (slept for ~{}s)",
                            elapsed.as_secs()
                        );

                        // Allow network interfaces (DHCP/Wi-Fi) 1.5s to settle
                        tokio::time::sleep(Duration::from_millis(1500)).await;
                        last_tick = std::time::Instant::now();

                        // Re-apply system proxy to currently active network services
                        let core_clone = core.clone();
                        tauri::async_runtime::spawn_blocking(move || {
                            let mut core_guard = core_clone.blocking_lock();
                            if let Err(e) = core_guard.reapply_system_proxy() {
                                eprintln!("[WakeMonitor] Failed to reapply system proxy on wake: {e:?}");
                            }
                        });

                        // Emit wake event to frontend
                        let _ = app.emit("system-wake", ());
                    }
                }
            }
        }
    });

    (handle, tx)
}
