pub mod error;

pub mod adapters;
pub mod core;
pub mod tray;
pub mod tray_icon;

#[cfg(target_os = "macos")]
use objc2::MainThreadMarker;
#[cfg(target_os = "macos")]
use objc2_app_kit::{NSColor, NSToolbar, NSWindow, NSWindowStyleMask, NSWindowTitleVisibility};
use std::sync::Arc;
use tauri::Manager;
use tokio::sync::Mutex;

use crate::core::dto::{
    CorePreflightReport, CoreRuntimeStatus, CreatePolicyInput, CreateProfileInput,
    CreateProxyInput, DnsHostEntry, OutboundMode, ProfileMeta, Proxy, ProxyPolicy, Rule,
    SelectPolicyMemberInput, SyncSummary, UpdatePolicyInput, UpdateProfileInput, UpdateProxyInput,
    ZeytunCore,
};
use crate::core::events::EventBus;
use crate::core::manager::CoreManager;

pub struct AppState {
    pub hide_on_close: std::sync::atomic::AtomicBool,
    core: Arc<Mutex<CoreManager>>,
}

impl AppState {
    /// Clone the shared core handle for use off the async runtime (tray menu
    /// handlers `blocking_lock` inside `spawn_blocking`).
    pub(crate) fn core_arc(&self) -> Arc<Mutex<CoreManager>> {
        self.core.clone()
    }
}

pub struct DaemonState {
    pub shutdown_tx: std::sync::Mutex<Option<tokio::sync::oneshot::Sender<()>>>,
    pub daemon_handle: std::sync::Mutex<Option<tauri::async_runtime::JoinHandle<()>>>,
}

/// Handle + shutdown channel for the background subscription auto-updater.
pub struct AutoUpdateState {
    pub shutdown_tx: std::sync::Mutex<Option<tokio::sync::oneshot::Sender<()>>>,
    pub task_handle: std::sync::Mutex<Option<tauri::async_runtime::JoinHandle<()>>>,
}

/// Temp rule expiry GC (sleep-until-next-expires, prune + reload).
pub struct TempRuleGcState {
    pub shutdown_tx: std::sync::Mutex<Option<tokio::sync::oneshot::Sender<()>>>,
    pub task_handle: std::sync::Mutex<Option<tauri::async_runtime::JoinHandle<()>>>,
}

pub struct NetworkTestState {
    pub task_handle: std::sync::Mutex<Option<tauri::async_runtime::JoinHandle<()>>>,
}

pub struct StunTestState {
    pub task_handle: std::sync::Mutex<Option<tauri::async_runtime::JoinHandle<()>>>,
}

/// Per-connection tracking for the Inspector window: live connections plus a
/// bounded ring buffer of recently-closed ones. Written by the daemon stream,
/// read by `core_inspector_snapshot` and the daemon emit task. `std::sync::Mutex`
/// — short critical sections, never held across `.await`.
pub struct InspectorState {
    pub active: std::sync::Mutex<std::collections::HashMap<String, crate::core::dto::ConnRecord>>,
    pub recent: std::sync::Mutex<std::collections::VecDeque<crate::core::dto::ConnRecord>>,
    /// Set whenever the connection set changes (new/update/close/reset/clear).
    /// The daemon emit task consumes it to push `inspector-snapshot` events only
    /// when something actually changed.
    pub dirty: std::sync::atomic::AtomicBool,
    /// Set by the Inspector route on mount/destroy (via
    /// `core_inspector_set_active`) — snapshots are pushed only while the page
    /// is actually open, even though the main window is always running.
    pub listener_active: std::sync::atomic::AtomicBool,
}

impl Default for InspectorState {
    fn default() -> Self {
        Self::new()
    }
}

impl InspectorState {
    pub fn new() -> Self {
        Self {
            active: std::sync::Mutex::new(std::collections::HashMap::new()),
            recent: std::sync::Mutex::new(std::collections::VecDeque::new()),
            dirty: std::sync::atomic::AtomicBool::new(true),
            listener_active: std::sync::atomic::AtomicBool::new(false),
        }
    }

    /// Full snapshot for the Inspector window: live connections (newest first)
    /// plus the closed ring buffer (newest first). Shared by the snapshot
    /// command (initial load) and the daemon emit task (push updates).
    pub fn snapshot(
        &self,
    ) -> Result<crate::core::dto::InspectorSnapshot, crate::error::CommandError> {
        use crate::core::dto::ConnDto;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);

        let active = {
            let guard = self.active.lock().map_err(|_| {
                crate::error::CommandError::Internal("inspector lock poisoned".into())
            })?;
            let mut v: Vec<ConnDto> = guard
                .values()
                .map(|r| ConnDto::from_record(r, now))
                .collect();
            v.sort_unstable_by_key(|a| std::cmp::Reverse(a.created_at));
            v
        };
        let recent = {
            let guard = self.recent.lock().map_err(|_| {
                crate::error::CommandError::Internal("inspector lock poisoned".into())
            })?;
            // Newest first.
            guard
                .iter()
                .rev()
                .map(|r| ConnDto::from_record(r, now))
                .collect()
        };

        Ok(crate::core::dto::InspectorSnapshot { active, recent })
    }
}

/// Max recently-closed connections retained (ring buffer).
pub const INSPECTOR_RECENT_CAP: usize = 1000;

/// Shared, lazily-connected gRPC channel to the zeytun-core daemon. `connect_lazy`
/// must run inside a Tokio runtime (it spawns a background connector), so the
/// channel is built on first command use rather than at app setup — the setup
/// hook runs on the main thread with no reactor.
pub struct GrpcState {
    channel: tokio::sync::OnceCell<tonic::transport::Channel>,
}

impl GrpcState {
    fn new() -> Self {
        Self {
            channel: tokio::sync::OnceCell::new(),
        }
    }

    async fn channel(&self) -> tonic::transport::Channel {
        self.channel
            .get_or_init(|| async {
                tonic::transport::Channel::from_static(crate::core::constants::GRPC_API_ENDPOINT)
                    .connect_lazy()
            })
            .await
            .clone()
    }
}

#[cfg(target_os = "macos")]
pub fn apply_mac_style(window: &tauri::WebviewWindow) {
    let ns_win_ptr = window.ns_window().unwrap();
    unsafe {
        let ns_window: &NSWindow = &*(ns_win_ptr as *const NSWindow);
        let mtm = MainThreadMarker::new().expect("UI must be updated on the main thread");

        let mut style_mask = ns_window.styleMask();
        style_mask.set(NSWindowStyleMask::FullSizeContentView, true);
        ns_window.setStyleMask(style_mask);

        let toolbar = NSToolbar::new(mtm);
        ns_window.setToolbar(Some(&toolbar));

        ns_window.setTitlebarAppearsTransparent(true);
        ns_window.setTitleVisibility(NSWindowTitleVisibility::Hidden);
        // ns_window.setTitle(ns_string!(""));
    }
}

/// Transparent frameless popup (connection-ask): clear NSWindow chrome.
#[cfg(target_os = "macos")]
pub fn apply_clear_window(window: &tauri::WebviewWindow) {
    let Ok(ns_win_ptr) = window.ns_window() else {
        return;
    };
    unsafe {
        let ns_window: &NSWindow = &*(ns_win_ptr as *const NSWindow);
        ns_window.setOpaque(false);
        ns_window.setBackgroundColor(Some(&NSColor::clearColor()));
        ns_window.setHasShadow(false);
    }
}

#[cfg(not(target_os = "macos"))]
pub fn apply_clear_window(_window: &tauri::WebviewWindow) {}

fn io_error(message: String) -> std::io::Error {
    std::io::Error::other(message)
}

/// Run a (potentially blocking) `CoreManager` operation off the async runtime.
///
/// `CoreManager` methods perform synchronous I/O (SQLite writes, zeytun-core
/// subprocess spawning, `ureq` network fetches). Running them directly under
/// `core.lock().await` would block a Tokio worker thread and serialize every
/// other command behind the held guard. Cloning the `Arc` and locking inside
/// `spawn_blocking` keeps the blocking work on the blocking pool.
async fn with_core_blocking<T, F>(
    state: &tauri::State<'_, AppState>,
    f: F,
) -> Result<T, crate::error::CommandError>
where
    F: FnOnce(&mut CoreManager) -> Result<T, crate::error::CommandError> + Send + 'static,
    T: Send + 'static,
{
    let core = state.core.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let mut guard = core.blocking_lock();
        f(&mut guard)
    })
    .await
    .map_err(|e| crate::error::CommandError::Internal(format!("task failed: {e}")))?
}

#[tauri::command]
async fn core_preflight(
    state: tauri::State<'_, AppState>,
) -> Result<CorePreflightReport, crate::error::CommandError> {
    with_core_blocking(&state, |core| Ok(core.preflight())).await
}

#[tauri::command]
async fn core_profile_get(
    state: tauri::State<'_, AppState>,
) -> Result<ZeytunCore, crate::error::CommandError> {
    let core = state.core.lock().await;
    Ok(core.profile_get().clone())
}

#[tauri::command]
async fn core_reset(
    state: tauri::State<'_, AppState>,
) -> Result<ZeytunCore, crate::error::CommandError> {
    with_core_blocking(&state, |core| core.reset()).await
}

#[tauri::command(rename_all = "snake_case")]
async fn core_proxy_import_link(
    link: String,
    state: tauri::State<'_, AppState>,
) -> Result<Proxy, crate::error::CommandError> {
    with_core_blocking(&state, move |core| core.proxy_import_link(&link)).await
}

#[tauri::command]
async fn core_proxy_create(
    input: CreateProxyInput,
    state: tauri::State<'_, AppState>,
) -> Result<Proxy, crate::error::CommandError> {
    with_core_blocking(&state, move |core| core.proxy_create(input)).await
}

#[tauri::command]
async fn core_start(
    state: tauri::State<'_, AppState>,
) -> Result<CoreRuntimeStatus, crate::error::CommandError> {
    let (status, is_global, clash_api) = with_core_blocking(&state, |core| {
        let status = core.start()?;
        let is_global = matches!(core.profile_get().outbound_mode, OutboundMode::Global);
        let clash_api = core.clash_api_client();
        Ok((status, is_global, clash_api))
    })
    .await?;

    if is_global {
        if let Some(clash_api) = clash_api {
            let policy_tag = crate::core::dto::DEFAULT_SELECTOR_POLICY_TAG.to_string();
            for _ in 0..crate::core::constants::CLASH_API_SELECT_RETRY_COUNT {
                tokio::time::sleep(std::time::Duration::from_millis(
                    crate::core::constants::CLASH_API_SELECT_RETRY_DELAY_MS,
                ))
                .await;
                if clash_api.select_proxy("GLOBAL", &policy_tag).is_ok() {
                    break;
                }
            }
        }
    }

    Ok(status)
}

#[tauri::command]
async fn core_stop(
    state: tauri::State<'_, AppState>,
) -> Result<CoreRuntimeStatus, crate::error::CommandError> {
    with_core_blocking(&state, |core| Ok(core.stop())).await
}

#[tauri::command]
async fn core_restart(
    state: tauri::State<'_, AppState>,
) -> Result<CoreRuntimeStatus, crate::error::CommandError> {
    with_core_blocking(&state, |core| core.restart()).await
}

#[tauri::command]
async fn core_status(
    state: tauri::State<'_, AppState>,
) -> Result<CoreRuntimeStatus, crate::error::CommandError> {
    let mut core = state.core.lock().await;
    Ok(core.status())
}

#[tauri::command]
fn core_event_list(
    bus: tauri::State<'_, Arc<EventBus>>,
) -> Result<Vec<crate::core::events::Event>, crate::error::CommandError> {
    Ok(bus.list())
}

#[tauri::command]
async fn core_get_traffic_analytics(
    filter: String,
    range: Option<String>,
    state: tauri::State<'_, AppState>,
) -> Result<crate::core::dto::TrafficAnalytics, crate::error::CommandError> {
    let proxy_only = filter == "proxy";
    // Rolling window, minute-aligned. Default 24h.
    let window_secs: i64 = match range.as_deref() {
        Some("1h") => 3600,
        Some("6h") => 6 * 3600,
        Some("7d") => 7 * 24 * 3600,
        Some("30d") => 30 * 24 * 3600,
        _ => 24 * 3600,
    };
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let since_ts = (now - window_secs) / 60 * 60;
    with_core_blocking(&state, move |core| {
        core.traffic_analytics(since_ts, proxy_only)
    })
    .await
}

#[tauri::command]
async fn core_get_traffic_summary(
    period: String,
    state: tauri::State<'_, AppState>,
) -> Result<crate::core::dto::TrafficSummary, crate::error::CommandError> {
    with_core_blocking(&state, move |core| core.traffic_summary(&period)).await
}

#[tauri::command]
fn core_inspector_set_active(
    inspector: tauri::State<'_, InspectorState>,
    active: bool,
) -> Result<(), crate::error::CommandError> {
    // Inspector is a route in the main window now (no separate window); the
    // daemon push feed follows this flag instead of window visibility.
    inspector
        .listener_active
        .store(active, std::sync::atomic::Ordering::Relaxed);
    Ok(())
}

#[tauri::command]
async fn core_open_connection_ask_window(
    app: tauri::AppHandle,
) -> Result<(), crate::error::CommandError> {
    use tauri::{WebviewUrl, WebviewWindowBuilder};

    // If the window already exists, just bring it to front
    if let Some(win) = app.get_webview_window("connection-ask") {
        let _ = win.set_size(tauri::Size::Logical(tauri::LogicalSize {
            width: 680.0,
            height: 650.0,
        }));
        #[cfg(target_os = "macos")]
        {
            let win_clone = win.clone();
            let _ = app.run_on_main_thread(move || {
                apply_clear_window(&win_clone);
            });
        }
        let _ = win.show();
        let _ = win.set_focus();
        return Ok(());
    }

    let window = WebviewWindowBuilder::new(
        &app,
        "connection-ask",
        WebviewUrl::App("connection-ask".into()),
    )
    .title("Connection Request")
    .inner_size(680.0, 650.0)
    .min_inner_size(680.0, 650.0)
    .max_inner_size(680.0, 650.0)
    .resizable(false)
    .always_on_top(true)
    .decorations(false)
    .transparent(true)
    .skip_taskbar(true)
    .visible(false)
    .build()
    .map_err(|e| crate::error::CommandError::Internal(e.to_string()))?;

    #[cfg(target_os = "macos")]
    {
        let win_clone = window.clone();
        let _ = app.run_on_main_thread(move || {
            apply_clear_window(&win_clone);
        });
    }
    Ok(())
}

#[tauri::command]
fn core_inspector_snapshot(
    inspector: tauri::State<'_, InspectorState>,
) -> Result<crate::core::dto::InspectorSnapshot, crate::error::CommandError> {
    inspector.snapshot()
}

#[tauri::command]
fn core_inspector_clear(
    inspector: tauri::State<'_, InspectorState>,
) -> Result<(), crate::error::CommandError> {
    if let Ok(mut recent) = inspector.recent.lock() {
        recent.clear();
        // Push the emptied state to an open Inspector window on the next tick
        // (overwrites any stale in-flight snapshot).
        inspector
            .dirty
            .store(true, std::sync::atomic::Ordering::Relaxed);
    }
    Ok(())
}

#[tauri::command]
async fn core_profile_list(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<ProfileMeta>, crate::error::CommandError> {
    with_core_blocking(&state, |core| core.profile_list()).await
}

#[tauri::command]
async fn core_profile_create(
    input: CreateProfileInput,
    state: tauri::State<'_, AppState>,
) -> Result<ProfileMeta, crate::error::CommandError> {
    with_core_blocking(&state, move |core| core.profile_create(input)).await
}

#[tauri::command]
async fn core_profile_update(
    input: UpdateProfileInput,
    state: tauri::State<'_, AppState>,
) -> Result<ProfileMeta, crate::error::CommandError> {
    with_core_blocking(&state, move |core| core.profile_update(input)).await
}

#[tauri::command(rename_all = "snake_case")]
async fn core_profile_delete(
    profile_id: String,
    state: tauri::State<'_, AppState>,
) -> Result<(), crate::error::CommandError> {
    with_core_blocking(&state, move |core| core.profile_delete(&profile_id)).await
}

#[tauri::command(rename_all = "snake_case")]
async fn core_profile_switch(
    profile_id: String,
    state: tauri::State<'_, AppState>,
    app: tauri::AppHandle,
) -> Result<(), crate::error::CommandError> {
    let snapshot = with_core_blocking(&state, move |core| {
        core.profile_switch(&profile_id)?;
        Ok(tray::TrayState::from_profile(core.profile_get()))
    })
    .await?;
    tray::sync_tray_ui(&app, snapshot);
    Ok(())
}

#[tauri::command(rename_all = "snake_case")]
async fn core_profile_refresh(
    profile_id: String,
    background: bool,
    state: tauri::State<'_, AppState>,
) -> Result<SyncSummary, crate::error::CommandError> {
    with_core_blocking(&state, move |core| {
        core.profile_refresh(&profile_id, background)
    })
    .await
}

#[tauri::command(rename_all = "snake_case")]
async fn core_profile_mark_summary_read(
    profile_id: String,
    state: tauri::State<'_, AppState>,
) -> Result<(), crate::error::CommandError> {
    with_core_blocking(&state, move |core| {
        core.profile_mark_summary_read(&profile_id)
    })
    .await
}

#[tauri::command]
async fn core_clash_proxies(
    state: tauri::State<'_, AppState>,
) -> Result<serde_json::Value, crate::error::CommandError> {
    let client = state
        .core
        .lock()
        .await
        .clash_api_client()
        .ok_or_else(|| "core is not running".to_string())?;

    let proxies = tauri::async_runtime::spawn_blocking(move || client.get_proxies())
        .await
        .map_err(|e| format!("task failed: {e}"))?
        .map_err(|e| crate::error::CommandError::Api(format!("clash api error: {e}")))?;

    serde_json::to_value(proxies).map_err(|e| crate::error::CommandError::Internal(e.to_string()))
}

// TODO: REMOVE IT

#[tauri::command]
fn set_hide_on_close(state: tauri::State<'_, AppState>, hide: bool) {
    state
        .hide_on_close
        .store(hide, std::sync::atomic::Ordering::Relaxed);
}

#[tauri::command]
async fn core_set_mode(
    mode: OutboundMode,
    state: tauri::State<'_, AppState>,
    app: tauri::AppHandle,
) -> Result<(), crate::error::CommandError> {
    let snapshot = with_core_blocking(&state, move |core| {
        core.set_mode(mode)?;
        Ok(tray::TrayState::from_profile(core.profile_get()))
    })
    .await?;
    tray::sync_tray_ui(&app, snapshot);
    Ok(())
}

#[tauri::command]
async fn core_set_inbound_mode(
    mode: String,
    state: tauri::State<'_, AppState>,
    app: tauri::AppHandle,
) -> Result<(), crate::error::CommandError> {
    let snapshot = with_core_blocking(&state, move |core| {
        core.set_inbound_mode(&mode)?;
        Ok(tray::TrayState::from_profile(core.profile_get()))
    })
    .await?;
    tray::sync_tray_ui(&app, snapshot);
    Ok(())
}

#[tauri::command]
async fn core_set_system_proxy(
    enabled: bool,
    state: tauri::State<'_, AppState>,
    app: tauri::AppHandle,
) -> Result<(), crate::error::CommandError> {
    let snapshot = with_core_blocking(&state, move |core| {
        core.set_system_proxy(enabled)?;
        Ok(tray::TrayState::from_profile(core.profile_get()))
    })
    .await?;
    tray::sync_tray_ui(&app, snapshot);
    Ok(())
}

#[tauri::command]
async fn core_set_allow_lan(
    enabled: bool,
    state: tauri::State<'_, AppState>,
) -> Result<(), crate::error::CommandError> {
    with_core_blocking(&state, move |core| {
        core.set_allow_lan(enabled)?;
        Ok(())
    })
    .await?;
    Ok(())
}

#[tauri::command]
async fn core_set_listen_port(
    port: u16,
    state: tauri::State<'_, AppState>,
) -> Result<(), crate::error::CommandError> {
    with_core_blocking(&state, move |core| {
        core.set_listen_port(port)?;
        Ok(())
    })
    .await?;
    Ok(())
}

#[tauri::command]
async fn core_set_log_level(
    level: String,
    state: tauri::State<'_, AppState>,
) -> Result<(), crate::error::CommandError> {
    with_core_blocking(&state, move |core| {
        core.set_log_level(level)?;
        Ok(())
    })
    .await?;
    Ok(())
}

#[tauri::command]
async fn core_update_proxy(
    input: UpdateProxyInput,
    state: tauri::State<'_, AppState>,
) -> Result<Proxy, crate::error::CommandError> {
    with_core_blocking(&state, move |core| core.update_proxy(input)).await
}

#[tauri::command(rename_all = "snake_case")]
async fn core_delete_proxy(
    proxy_id: String,
    state: tauri::State<'_, AppState>,
) -> Result<(), crate::error::CommandError> {
    with_core_blocking(&state, move |core| core.delete_proxy(&proxy_id)).await
}

#[tauri::command(rename_all = "snake_case")]
async fn core_proxy_export_link(
    proxy_id: String,
    state: tauri::State<'_, AppState>,
) -> Result<String, crate::error::CommandError> {
    with_core_blocking(&state, move |core| core.proxy_export_link(&proxy_id)).await
}

#[tauri::command]
async fn core_create_policy(
    input: CreatePolicyInput,
    state: tauri::State<'_, AppState>,
) -> Result<ProxyPolicy, crate::error::CommandError> {
    with_core_blocking(&state, move |core| core.create_policy(input)).await
}

#[tauri::command]
async fn core_update_policy(
    input: UpdatePolicyInput,
    state: tauri::State<'_, AppState>,
) -> Result<ProxyPolicy, crate::error::CommandError> {
    with_core_blocking(&state, move |core| core.update_policy(input)).await
}

#[tauri::command(rename_all = "snake_case")]
async fn core_delete_policy(
    policy_id: String,
    state: tauri::State<'_, AppState>,
) -> Result<(), crate::error::CommandError> {
    with_core_blocking(&state, move |core| core.delete_policy(&policy_id)).await
}

#[tauri::command]
async fn core_select_policy_member(
    input: SelectPolicyMemberInput,
    state: tauri::State<'_, AppState>,
) -> Result<(), crate::error::CommandError> {
    with_core_blocking(&state, move |core| core.select_policy_member(input)).await
}

#[tauri::command]
async fn core_create_ruleset(
    kind: String,
    source: String,
    action: String,
    comment: Option<String>,
    file_bytes: Option<Vec<u8>>,
    state: tauri::State<'_, AppState>,
    app: tauri::AppHandle,
) -> Result<crate::core::dto::RuleSet, crate::error::CommandError> {
    let app_data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| crate::error::CommandError::Internal(e.to_string()))?;

    with_core_blocking(&state, move |core| {
        core.create_ruleset(app_data_dir, kind, source, action, comment, file_bytes)
    })
    .await
}

#[tauri::command(rename_all = "snake_case")]
async fn core_stage_ruleset_file(
    file_bytes: Vec<u8>,
    app: tauri::AppHandle,
) -> Result<String, crate::error::CommandError> {
    let app_data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| crate::error::CommandError::Internal(e.to_string()))?;
    crate::core::manager::CoreManager::stage_ruleset_file(app_data_dir, file_bytes)
}

#[tauri::command(rename_all = "snake_case")]
async fn core_update_rulesets(
    rule_sets: Vec<crate::core::dto::RuleSet>,
    state: tauri::State<'_, AppState>,
) -> Result<(), crate::error::CommandError> {
    with_core_blocking(&state, move |core| core.update_rulesets(rule_sets)).await
}

#[tauri::command(rename_all = "snake_case")]
async fn core_update_dns_rules(
    dns_rules: Vec<crate::core::dto::DnsRule>,
    state: tauri::State<'_, AppState>,
) -> Result<(), crate::error::CommandError> {
    with_core_blocking(&state, move |core| core.update_dns_rules(dns_rules)).await
}

#[tauri::command(rename_all = "snake_case")]
async fn core_update_dns_final(
    final_server: Option<String>,
    state: tauri::State<'_, AppState>,
) -> Result<(), crate::error::CommandError> {
    with_core_blocking(&state, move |core| core.update_dns_final(final_server)).await
}

#[tauri::command(rename_all = "snake_case")]
async fn core_update_dns_hosts(
    hosts: Vec<DnsHostEntry>,
    state: tauri::State<'_, AppState>,
) -> Result<(), crate::error::CommandError> {
    with_core_blocking(&state, move |core| core.update_dns_hosts(hosts)).await
}

#[tauri::command]
async fn core_read_active_config(
    state: tauri::State<'_, AppState>,
) -> Result<String, crate::error::CommandError> {
    let core = state.core.lock().await;
    let base_dir = core.paths_info().config_dir.clone();
    let path = std::path::Path::new(&base_dir)
        .join("zeytun-core")
        .join("Default.json");

    std::fs::read_to_string(&path).map_err(|e| {
        crate::error::CommandError::Internal(format!(
            "Failed to read config file at {}: {e}",
            path.display()
        ))
    })
}

#[tauri::command]
async fn core_delete_ruleset(
    id: String,
    state: tauri::State<'_, AppState>,
) -> Result<(), crate::error::CommandError> {
    with_core_blocking(&state, move |core| core.delete_ruleset(id)).await
}

#[tauri::command(rename_all = "snake_case")]
async fn core_retry_rulesets(
    policy_tag: String,
    state: tauri::State<'_, AppState>,
) -> Result<(), crate::error::CommandError> {
    with_core_blocking(&state, move |core| core.retry_rulesets(policy_tag)).await
}

#[tauri::command]
async fn core_update_rules(
    rules: Vec<Rule>,
    state: tauri::State<'_, AppState>,
) -> Result<(), crate::error::CommandError> {
    with_core_blocking(&state, move |core| core.update_rules(rules)).await
}

#[tauri::command(rename_all = "snake_case")]
async fn core_update_temp_rules(
    temp_rules: Vec<crate::core::dto::TempRule>,
    state: tauri::State<'_, AppState>,
) -> Result<(), crate::error::CommandError> {
    with_core_blocking(&state, move |core| core.update_temp_rules(temp_rules)).await
}

#[tauri::command]
async fn core_prune_expired_temp_rules(
    state: tauri::State<'_, AppState>,
) -> Result<bool, crate::error::CommandError> {
    with_core_blocking(&state, move |core| core.prune_expired_temp_rules()).await
}

#[tauri::command(rename_all = "snake_case")]
async fn core_promote_temp_rule(
    id: u64,
    state: tauri::State<'_, AppState>,
) -> Result<(), crate::error::CommandError> {
    with_core_blocking(&state, move |core| core.promote_temp_rule(id)).await
}

#[tauri::command(rename_all = "snake_case")]
async fn core_set_connection_ask(
    config: crate::core::dto::ConnectionAskConfig,
    state: tauri::State<'_, AppState>,
) -> Result<(), crate::error::CommandError> {
    with_core_blocking(&state, move |core| core.set_connection_ask(config)).await
}

/// Route chosen for each app-originated traffic class. Traffic classes missing
/// from the map are Direct — the frontend must not assume every key is present.
#[tauri::command(rename_all = "snake_case")]
async fn core_network_policy_get(
    state: tauri::State<'_, AppState>,
) -> Result<
    std::collections::HashMap<String, crate::core::network_policy::NetworkPolicy>,
    crate::error::CommandError,
> {
    with_core_blocking(&state, move |core| core.network_policies()).await
}

#[tauri::command(rename_all = "snake_case")]
async fn core_network_policy_set(
    traffic: String,
    policy: crate::core::network_policy::NetworkPolicy,
    state: tauri::State<'_, AppState>,
) -> Result<(), crate::error::CommandError> {
    with_core_blocking(&state, move |core| {
        core.set_network_policy(&traffic, &policy)
    })
    .await
}

#[tauri::command(rename_all = "snake_case")]
async fn core_decide_connection_ask(
    id: String,
    outbound: String,
    reject: bool,
    remember: bool,
    process_path: Option<String>,
    process_bundle: Option<String>,
    dest_host: Option<String>,
    group_by: Option<String>,
    state: tauri::State<'_, AppState>,
) -> Result<(), crate::error::CommandError> {
    with_core_blocking(&state, move |core| {
        core.decide_connection_ask(
            id,
            outbound,
            reject,
            remember,
            process_path,
            process_bundle,
            dest_host,
            group_by,
        )
    })
    .await
}

#[tauri::command]
async fn core_clash_select_proxy(
    group: String,
    proxy: String,
    state: tauri::State<'_, AppState>,
) -> Result<(), crate::error::CommandError> {
    let client = state
        .core
        .lock()
        .await
        .clash_api_client()
        .ok_or_else(|| "clash api is not available".to_string())?;

    tauri::async_runtime::spawn_blocking(move || client.select_proxy(&group, &proxy))
        .await
        .map_err(|e| format!("task failed: {e}"))?
        .map_err(|e| crate::error::CommandError::Api(format!("clash api error: {e}")))
}

#[derive(serde::Serialize)]
struct DnsLookupResult {
    resolved_ips: Vec<String>,
    server_used: String,
    latency_ms: u64,
    status: u16,
    record_type: String,
    ttl: Option<u32>,
}

#[tauri::command]
async fn test_dns_lookup(
    domain: String,
    state: tauri::State<'_, AppState>,
) -> Result<DnsLookupResult, crate::error::CommandError> {
    let domain = crate::core::manager::normalize_dns_domain(&domain)?;
    let client = {
        let core = state.core.lock().await;
        if !core.is_running() {
            return Err(crate::error::CommandError::State(
                "core is not running".to_string(),
            ));
        }
        core.clash_api_client().ok_or_else(|| {
            crate::error::CommandError::Api("clash api is not available".to_string())
        })?
    };

    let (response, latency_ms) = tauri::async_runtime::spawn_blocking(move || {
        let started = std::time::Instant::now();
        let response = client.dns_query(&domain);
        (response, started.elapsed().as_millis() as u64)
    })
    .await
    .map_err(|e| crate::error::CommandError::Internal(format!("task failed: {e}")))?;
    let response =
        response.map_err(|e| crate::error::CommandError::Api(format!("clash api error: {e}")))?;

    let mut resolved_ips = Vec::new();
    let mut ttl = None;
    for answer in response.answer {
        if answer.record_type != 1 {
            continue;
        }
        if let Ok(ip) = answer.data.trim().parse::<std::net::Ipv4Addr>() {
            resolved_ips.push(ip.to_string());
            ttl.get_or_insert(answer.ttl);
        }
    }

    Ok(DnsLookupResult {
        resolved_ips,
        server_used: response.server,
        latency_ms,
        status: response.status,
        record_type: "A".to_string(),
        ttl,
    })
}

#[tauri::command]
async fn flush_dns_cache(
    state: tauri::State<'_, AppState>,
) -> Result<String, crate::error::CommandError> {
    let client = {
        let core = state.core.lock().await;
        if !core.is_running() {
            return Err(crate::error::CommandError::State(
                "core is not running".to_string(),
            ));
        }
        core.clash_api_client().ok_or_else(|| {
            crate::error::CommandError::Api("clash api is not available".to_string())
        })?
    };

    tauri::async_runtime::spawn_blocking(move || client.flush_dns_cache())
        .await
        .map_err(|e| crate::error::CommandError::Internal(format!("task failed: {e}")))?
        .map_err(|e| crate::error::CommandError::Api(format!("clash api error: {e}")))?;
    Ok("DNS and FakeIP caches flushed successfully".to_string())
}

#[tauri::command]
async fn core_clash_set_mode(
    mode: String,
    state: tauri::State<'_, AppState>,
) -> Result<(), crate::error::CommandError> {
    let client = state
        .core
        .lock()
        .await
        .clash_api_client()
        .ok_or_else(|| "clash api is not available".to_string())?;

    tauri::async_runtime::spawn_blocking(move || client.patch_config(&mode))
        .await
        .map_err(|e| format!("task failed: {e}"))?
        .map_err(|e| crate::error::CommandError::Api(format!("clash api error: {e}")))
}

#[derive(serde::Serialize)]
struct ClashRule {
    #[serde(rename = "type")]
    rule_type: &'static str,
    payload: &'static str,
    proxy: String,
}

#[derive(serde::Serialize)]
struct RulesResponse {
    rules: Vec<ClashRule>,
}

#[tauri::command]
async fn core_rules(
    state: tauri::State<'_, AppState>,
) -> Result<RulesResponse, crate::error::CommandError> {
    let rules: Vec<ClashRule> = {
        let core = state.core.lock().await;
        core.profile_get()
            .rules
            .iter()
            .map(|rule| ClashRule {
                rule_type: "Rule",
                payload: "",
                proxy: rule.outbound.clone(),
            })
            .collect()
    };
    Ok(RulesResponse { rules })
}

#[tauri::command]
async fn core_clash_proxy_delay_test(
    proxy: String,
    url: String,
    timeout_ms: u64,
    state: tauri::State<'_, AppState>,
) -> Result<serde_json::Value, crate::error::CommandError> {
    let client = state
        .core
        .lock()
        .await
        .clash_api_client()
        .ok_or_else(|| "core is not running".to_string())?;

    let result = tauri::async_runtime::spawn_blocking(move || {
        client.proxy_delay_test(&proxy, &url, timeout_ms)
    })
    .await
    .map_err(|e| format!("task failed: {e}"))?
    .map_err(|e| crate::error::CommandError::Api(format!("clash api error: {e}")))?;

    serde_json::to_value(result).map_err(|e| crate::error::CommandError::Internal(e.to_string()))
}

#[derive(serde::Serialize)]
struct ClashApiConfig {
    controller: String,
    secret: Option<String>,
}

#[tauri::command]
async fn core_clash_api_config(
    state: tauri::State<'_, AppState>,
) -> Result<serde_json::Value, crate::error::CommandError> {
    let client = state
        .core
        .lock()
        .await
        .clash_api_client()
        .ok_or_else(|| "core is not running".to_string())?;

    let config = ClashApiConfig {
        controller: client.controller().to_string(),
        secret: client.secret().map(|s| s.to_string()),
    };

    serde_json::to_value(config).map_err(|e| crate::error::CommandError::Internal(e.to_string()))
}

#[tauri::command]
async fn get_process_icon(
    process_name: String,
    process_path: String,
    app: tauri::AppHandle,
) -> Result<String, crate::error::CommandError> {
    let cache_dir = app
        .path()
        .app_cache_dir()
        .map_err(|e| crate::error::CommandError::Internal(e.to_string()))?;

    // The traffic card's process tab lists names from historical analytics,
    // which may have no live connection (hence no path). Resolve the app
    // bundle from the name so the icon still appears.
    let process_path = if process_path.is_empty() {
        crate::adapters::process_resolve::resolve_process("PROCESS-NAME", &process_name).1
    } else {
        process_path
    };

    // Perform extraction inside spawn_blocking so AppKit work doesn't block Tokio
    tauri::async_runtime::spawn_blocking(move || {
        let path = crate::adapters::process_icon::ensure_process_icon(
            &cache_dir,
            &process_name,
            &process_path,
        )?;
        Ok(path.to_string_lossy().into_owned())
    })
    .await
    .map_err(|e| crate::error::CommandError::Internal(e.to_string()))?
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct ResolvedProcess {
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    icon_path: Option<String>,
}

/// Resolve a process-style rule value (PROCESS-NAME / PROCESS-PATH /
/// PROCESS-PATH-REGEX) to a display name and, when an app bundle is found,
/// the cached icon PNG path (loaded via `convertFileSrc` on the frontend).
/// Best-effort: never errors; degrades to name-only (terminal fallback icon).
#[tauri::command]
async fn core_resolve_process(
    kind: String,
    value: String,
    app: tauri::AppHandle,
) -> Result<Option<ResolvedProcess>, crate::error::CommandError> {
    let (name, path) = crate::adapters::process_resolve::resolve_process(&kind, &value);

    if path.is_empty() {
        return Ok(Some(ResolvedProcess {
            name,
            icon_path: None,
        }));
    }

    let cache_dir = app
        .path()
        .app_cache_dir()
        .map_err(|e| crate::error::CommandError::Internal(e.to_string()))?;

    let cache_name = name.clone();
    let cache_path = path.clone();
    let icon_path = tauri::async_runtime::spawn_blocking(move || {
        crate::adapters::process_icon::ensure_process_icon(&cache_dir, &cache_name, &cache_path)
            .map(|p| p.to_string_lossy().into_owned())
    })
    .await
    .map_err(|e| crate::error::CommandError::Internal(e.to_string()))?;

    // Icon extraction is best-effort; a failure still shows name + fallback icon.
    let icon_path = icon_path.ok();
    Ok(Some(ResolvedProcess { name, icon_path }))
}

#[derive(serde::Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct NetworkQualityTestProgressEvent {
    pub phase: i32,
    pub download_capacity: i64,
    pub upload_capacity: i64,
    pub download_rpm: i32,
    pub upload_rpm: i32,
    pub idle_latency_ms: i32,
    pub elapsed_ms: i64,
    pub is_final: bool,
    pub error: String,
    pub download_capacity_accuracy: i32,
    pub upload_capacity_accuracy: i32,
    pub download_rpm_accuracy: i32,
    pub upload_rpm_accuracy: i32,
}

#[tauri::command]
async fn core_network_quality_test_start(
    config_url: String,
    outbound_tag: String,
    serial: bool,
    max_runtime_seconds: i32,
    http3: bool,
    app: tauri::AppHandle,
) -> Result<(), crate::error::CommandError> {
    // Cancel any existing test
    if let Some(state) = app.try_state::<NetworkTestState>() {
        if let Ok(mut handle) = state.task_handle.lock() {
            if let Some(h) = handle.take() {
                h.abort();
            }
        }
    }

    let grpc = app.try_state::<GrpcState>().ok_or_else(|| {
        crate::error::CommandError::State("grpc channel not initialized".to_string())
    })?;
    let channel = grpc.channel().await;

    let mut client =
        crate::core::daemon::pb::started_service_client::StartedServiceClient::with_interceptor(
            channel,
            crate::core::daemon::AuthInterceptor,
        );

    let req = tonic::Request::new(crate::core::daemon::pb::NetworkQualityTestRequest {
        config_url,
        outbound_tag,
        serial,
        max_runtime_seconds,
        http3,
    });

    let mut stream = client
        .start_network_quality_test(req)
        .await
        .map_err(|e| crate::error::CommandError::Internal(e.to_string()))?
        .into_inner();

    let app_clone = app.clone();
    let handle = tauri::async_runtime::spawn(async move {
        use tauri::Emitter;
        while let Ok(Some(progress)) = stream.message().await {
            let is_final = progress.is_final;
            let has_error = !progress.error.is_empty();

            let event = NetworkQualityTestProgressEvent {
                phase: progress.phase,
                download_capacity: progress.download_capacity,
                upload_capacity: progress.upload_capacity,
                download_rpm: progress.download_rpm,
                upload_rpm: progress.upload_rpm,
                idle_latency_ms: progress.idle_latency_ms,
                elapsed_ms: progress.elapsed_ms,
                is_final: progress.is_final,
                error: progress.error,
                download_capacity_accuracy: progress.download_capacity_accuracy,
                upload_capacity_accuracy: progress.upload_capacity_accuracy,
                download_rpm_accuracy: progress.download_rpm_accuracy,
                upload_rpm_accuracy: progress.upload_rpm_accuracy,
            };
            let _ = app_clone.emit("network-test-update", event);
            if is_final || has_error {
                break;
            }
        }

        // Clean up handle when task completes
        if let Some(state) = app_clone.try_state::<NetworkTestState>() {
            if let Ok(mut h) = state.task_handle.lock() {
                *h = None;
            }
        }
    });

    // Store the handle
    if let Some(state) = app.try_state::<NetworkTestState>() {
        if let Ok(mut h) = state.task_handle.lock() {
            *h = Some(handle);
        }
    }

    Ok(())
}

#[tauri::command]
async fn core_network_quality_test_cancel(
    app: tauri::AppHandle,
) -> Result<(), crate::error::CommandError> {
    if let Some(state) = app.try_state::<NetworkTestState>() {
        if let Ok(mut handle) = state.task_handle.lock() {
            if let Some(h) = handle.take() {
                h.abort();
            }
        }
    }
    Ok(())
}

#[derive(serde::Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct StunTestProgressEvent {
    pub phase: i32,
    pub external_addr: String,
    pub latency_ms: i32,
    pub nat_mapping: i32,
    pub nat_filtering: i32,
    pub is_final: bool,
    pub error: String,
    pub nat_type_supported: bool,
}

#[tauri::command]
async fn core_stun_test_start(
    server: String,
    outbound_tag: String,
    app: tauri::AppHandle,
) -> Result<(), crate::error::CommandError> {
    // Cancel any existing test
    if let Some(state) = app.try_state::<StunTestState>() {
        if let Ok(mut handle) = state.task_handle.lock() {
            if let Some(h) = handle.take() {
                h.abort();
            }
        }
    }

    let grpc = app.try_state::<GrpcState>().ok_or_else(|| {
        crate::error::CommandError::State("grpc channel not initialized".to_string())
    })?;
    let channel = grpc.channel().await;

    let mut client =
        crate::core::daemon::pb::started_service_client::StartedServiceClient::with_interceptor(
            channel,
            crate::core::daemon::AuthInterceptor,
        );

    let req = tonic::Request::new(crate::core::daemon::pb::StunTestRequest {
        server,
        outbound_tag,
    });

    let mut stream = client
        .start_stun_test(req)
        .await
        .map_err(|e| crate::error::CommandError::Internal(e.to_string()))?
        .into_inner();

    let app_clone = app.clone();
    let handle = tauri::async_runtime::spawn(async move {
        use tauri::Emitter;
        while let Ok(Some(progress)) = stream.message().await {
            let is_final = progress.is_final;
            let has_error = !progress.error.is_empty();

            let event = StunTestProgressEvent {
                phase: progress.phase,
                external_addr: progress.external_addr,
                latency_ms: progress.latency_ms,
                nat_mapping: progress.nat_mapping,
                nat_filtering: progress.nat_filtering,
                is_final: progress.is_final,
                error: progress.error,
                nat_type_supported: progress.nat_type_supported,
            };
            let _ = app_clone.emit("stun-test-update", event);
            if is_final || has_error {
                break;
            }
        }

        if let Some(state) = app_clone.try_state::<StunTestState>() {
            if let Ok(mut h) = state.task_handle.lock() {
                *h = None;
            }
        }
    });

    if let Some(state) = app.try_state::<StunTestState>() {
        if let Ok(mut h) = state.task_handle.lock() {
            *h = Some(handle);
        }
    }

    Ok(())
}

#[tauri::command]
async fn core_stun_test_cancel(app: tauri::AppHandle) -> Result<(), crate::error::CommandError> {
    if let Some(state) = app.try_state::<StunTestState>() {
        if let Ok(mut handle) = state.task_handle.lock() {
            if let Some(h) = handle.take() {
                h.abort();
            }
        }
    }
    Ok(())
}

#[tauri::command]
async fn core_close_connections(
    ids: Vec<String>,
    grpc: tauri::State<'_, GrpcState>,
) -> Result<(), crate::error::CommandError> {
    let channel = grpc.channel().await;

    let mut client =
        crate::core::daemon::pb::started_service_client::StartedServiceClient::with_interceptor(
            channel,
            crate::core::daemon::AuthInterceptor,
        );

    for id in ids {
        let req = tonic::Request::new(crate::core::daemon::pb::CloseConnectionRequest { id });
        let _ = client.close_connection(req).await;
    }

    Ok(())
}

#[tauri::command]
async fn core_get_local_latencies() -> Result<serde_json::Value, crate::error::CommandError> {
    tauri::async_runtime::spawn_blocking(crate::adapters::net_probe::local_latencies)
        .await
        .map_err(|e| crate::error::CommandError::Internal(format!("task failed: {e}")))?
}

#[tauri::command]
async fn core_get_local_network_info(
) -> Result<crate::adapters::net_probe::LocalNetworkInfo, crate::error::CommandError> {
    tauri::async_runtime::spawn_blocking(crate::adapters::net_probe::local_network_info)
        .await
        .map_err(|e| crate::error::CommandError::Internal(format!("task failed: {e}")))?
}

#[tauri::command]
async fn core_get_active_network_interface() -> Result<String, crate::error::CommandError> {
    tauri::async_runtime::spawn_blocking(crate::adapters::net_probe::active_interface)
        .await
        .map_err(|e| crate::error::CommandError::Internal(format!("task failed: {e}")))?
}

/// Run the full network diagnostics suite (router ping, DNS timing, direct +
/// proxy HTTP probes) and return both the raw numbers for the dashboard card
/// and the formatted plaintext report for the diagnostics sheet.
///
/// The proxy leg only runs in Global mode with the core running; the mixed
/// inbound port is read from the active profile. All blocking network work runs
/// on the blocking pool so the UI never freezes.
#[tauri::command]
async fn core_run_diagnostics(
    internet_test_url: String,
    proxy_test_url: String,
    state: tauri::State<'_, AppState>,
) -> Result<crate::adapters::diagnostics::DiagnosticsResult, crate::error::CommandError> {
    let (mode, proxy_port, proxy_available) = {
        let core = state.core.lock().await;
        let profile = core.profile_get();
        let mode = match profile.outbound_mode {
            OutboundMode::Direct => "direct",
            OutboundMode::Global => "global",
            OutboundMode::Rule => "rule",
        };
        let proxy_port = profile.local_proxy.mixed_port.unwrap_or(6060);
        // Only probe the proxy when it can actually serve traffic: Global mode
        // (the whole tunnel is proxied) and the core is up. Rule mode is excluded
        // on purpose — the probe would traverse the routing table and could land
        // on direct, so it wouldn't reflect the selected proxy.
        let proxy_available =
            matches!(profile.outbound_mode, OutboundMode::Global) && core.is_running();
        (mode.to_string(), proxy_port, proxy_available)
    };

    let result = tauri::async_runtime::spawn_blocking(move || {
        crate::adapters::diagnostics::run(
            &mode,
            proxy_port,
            proxy_available,
            &internet_test_url,
            &proxy_test_url,
        )
    })
    .await
    .map_err(|e| crate::error::CommandError::Internal(e.to_string()))?;

    Ok(result)
}

#[tauri::command]
async fn core_get_external_ip_info(
    state: tauri::State<'_, AppState>,
    grpc: tauri::State<'_, GrpcState>,
    app: tauri::AppHandle,
) -> Result<crate::adapters::ip_info::IpInfo, crate::error::CommandError> {
    let (is_running, outbound_mode, proxy_port) = {
        let core = state.core.lock().await;
        let profile = core.profile_get();
        (
            core.is_running(),
            profile.outbound_mode,
            profile.local_proxy.mixed_port.unwrap_or(6060),
        )
    };

    let active_proxy_port =
        if is_running && matches!(outbound_mode, crate::core::dto::OutboundMode::Global) {
            Some(proxy_port)
        } else {
            None
        };

    let ip = if is_running {
        let outbound_tag = match outbound_mode {
            crate::core::dto::OutboundMode::Global => {
                crate::core::dto::DEFAULT_SELECTOR_POLICY_TAG.to_string()
            }
            _ => "direct".to_string(),
        };

        let channel = grpc.channel().await;
        let mut client =
            crate::core::daemon::pb::started_service_client::StartedServiceClient::with_interceptor(
                channel,
                crate::core::daemon::AuthInterceptor,
            );
        let req = tonic::Request::new(crate::core::daemon::pb::StunTestRequest {
            server: crate::core::constants::DEFAULT_STUN_SERVER.to_string(),
            outbound_tag,
        });

        let stun = tokio::time::timeout(std::time::Duration::from_secs(3), async {
            let response = client.start_stun_test(req).await.ok()?;
            let mut stream = response.into_inner();
            while let Ok(Some(progress)) = stream.message().await {
                if !progress.external_addr.is_empty() {
                    let addr = progress.external_addr;
                    let ip = addr
                        .rsplit_once(':')
                        .map(|(ip, _)| ip)
                        .unwrap_or(&addr)
                        .trim_start_matches('[')
                        .trim_end_matches(']');
                    return Some(ip.to_string());
                }
                if progress.is_final || !progress.error.is_empty() {
                    break;
                }
            }
            None
        })
        .await
        .ok()
        .flatten();
        if let Some(ip) = stun {
            ip
        } else {
            tauri::async_runtime::spawn_blocking(move || {
                crate::adapters::ip_info::fetch_external_ip(active_proxy_port)
            })
            .await
            .map_err(|e| crate::error::CommandError::Internal(e.to_string()))??
        }
    } else {
        tauri::async_runtime::spawn_blocking(move || {
            crate::adapters::ip_info::fetch_external_ip(active_proxy_port)
        })
        .await
        .map_err(|e| crate::error::CommandError::Internal(e.to_string()))??
    };

    let resource_dir = app
        .path()
        .resource_dir()
        .map_err(|e| crate::error::CommandError::Internal(e.to_string()))?;
    let app_data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| crate::error::CommandError::Internal(e.to_string()))?;
    let resource_path =
        crate::adapters::geoip_update::resolve_db_path(&app_data_dir, &resource_dir);

    crate::adapters::ip_info::ip_info(ip, &resource_path)
}

/// Download the latest GeoIP country database, honouring the `geoip` route.
#[tauri::command(rename_all = "snake_case")]
async fn core_update_geoip_db(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<crate::adapters::geoip_update::GeoipUpdate, crate::error::CommandError> {
    let proxy = {
        let core = state.core.lock().await;
        core.proxy_for_traffic(crate::core::network_policy::TRAFFIC_GEOIP)
    };
    let dest = app
        .path()
        .app_data_dir()
        .map_err(|e| crate::error::CommandError::Internal(e.to_string()))?
        .join(crate::core::constants::GEOIP_DB_FILENAME);

    tauri::async_runtime::spawn_blocking(move || {
        crate::adapters::geoip_update::update_country_db(&dest, proxy.as_deref())
            .map_err(crate::error::CommandError::Internal)
    })
    .await
    .map_err(|e| crate::error::CommandError::Internal(e.to_string()))?
}

/// Background GeoIP refresh: HEAD-probe the newest published month and download
/// only when it is newer than what's installed. Cheap enough to call on launch.
///
/// Returns a [`GeoipAutoUpdate`] rather than an error so the frontend can stay
/// silent on "already current" / "CDN unreachable" and only toast on a real
/// change — a per-launch network hiccup must never surface as a failure.
#[tauri::command(rename_all = "snake_case")]
async fn core_auto_update_geoip_db(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<crate::adapters::geoip_update::GeoipAutoUpdate, crate::error::CommandError> {
    let proxy = {
        let core = state.core.lock().await;
        core.proxy_for_traffic(crate::core::network_policy::TRAFFIC_GEOIP)
    };
    let dest = app
        .path()
        .app_data_dir()
        .map_err(|e| crate::error::CommandError::Internal(e.to_string()))?
        .join(crate::core::constants::GEOIP_DB_FILENAME);

    tauri::async_runtime::spawn_blocking(move || {
        crate::adapters::geoip_update::auto_update_country_db(&dest, proxy.as_deref())
    })
    .await
    .map_err(|e| crate::error::CommandError::Internal(e.to_string()))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_autostart::Builder::new().build())
        .setup(|app| {
            let window = app.get_webview_window("main").unwrap();

            #[cfg(target_os = "macos")]
            apply_mac_style(&window);

            if let Some(ask) = app.get_webview_window("connection-ask") {
                #[cfg(target_os = "macos")]
                apply_clear_window(&ask);
            }

            let app_data_dir = app
                .path()
                .app_data_dir()
                .map_err(|err| io_error(format!("failed to read app_data_dir: {}", err)))?;
            let resource_dir = app.path().resource_dir().ok();

            let event_bus = Arc::new(EventBus::new(app.handle().clone()));
            app.manage(event_bus.clone());

            let core_manager = CoreManager::new(app_data_dir, resource_dir, event_bus)
                .map_err(|err| io_error(format!("failed to initialize core manager: {}", err)))?;

            let core_arc = Arc::new(Mutex::new(core_manager));
            app.manage(AppState {
                core: core_arc.clone(),
                hide_on_close: std::sync::atomic::AtomicBool::new(true),
            });

            let (daemon_handle, shutdown_tx) =
                crate::core::daemon::start_daemon_listener(app.handle().clone());
            app.manage(DaemonState {
                shutdown_tx: std::sync::Mutex::new(Some(shutdown_tx)),
                daemon_handle: std::sync::Mutex::new(Some(daemon_handle)),
            });

            // Background subscription auto-updater: ticks on startup and hourly.
            let (au_handle, au_shutdown) = crate::core::auto_update::start_auto_updater(
                app.handle().clone(),
                core_arc.clone(),
            );
            app.manage(AutoUpdateState {
                shutdown_tx: std::sync::Mutex::new(Some(au_shutdown)),
                task_handle: std::sync::Mutex::new(Some(au_handle)),
            });

            let (gc_handle, gc_shutdown) =
                crate::core::temp_rule_gc::start_temp_rule_gc(app.handle().clone(), core_arc);
            app.manage(TempRuleGcState {
                shutdown_tx: std::sync::Mutex::new(Some(gc_shutdown)),
                task_handle: std::sync::Mutex::new(Some(gc_handle)),
            });

            app.manage(NetworkTestState {
                task_handle: std::sync::Mutex::new(None),
            });

            app.manage(StunTestState {
                task_handle: std::sync::Mutex::new(None),
            });

            app.manage(GrpcState::new());
            app.manage(InspectorState::new());

            // Seed the tray checkmarks from the loaded profile, then build it.
            let init = {
                let state = app.state::<AppState>();
                let core = tauri::async_runtime::block_on(state.core.lock());
                tray::TrayState::from_profile(core.profile_get())
            };
            tray::build_tray(app.handle(), &init)?;

            Ok(())
        })
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_websocket::init())
        .invoke_handler(tauri::generate_handler![
            core_preflight,
            core_profile_get,
            core_reset,
            core_proxy_import_link,
            core_proxy_create,
            core_start,
            core_stop,
            core_restart,
            core_status,
            core_event_list,
            core_get_traffic_analytics,
            core_get_traffic_summary,
            set_hide_on_close,
            core_inspector_set_active,
            core_inspector_snapshot,
            core_inspector_clear,
            core_profile_list,
            core_profile_create,
            core_profile_update,
            core_profile_delete,
            core_profile_switch,
            core_profile_refresh,
            core_profile_mark_summary_read,
            core_clash_proxies,
            core_clash_select_proxy,
            core_clash_set_mode,
            core_set_mode,
            core_set_inbound_mode,
            core_set_system_proxy,
            core_set_allow_lan,
            core_set_log_level,
            core_set_listen_port,
            core_update_proxy,
            core_delete_proxy,
            core_proxy_export_link,
            core_create_policy,
            core_update_policy,
            core_delete_policy,
            core_select_policy_member,
            core_create_ruleset,
            core_stage_ruleset_file,
            core_update_rulesets,
            core_update_dns_rules,
            core_update_dns_final,
            core_update_dns_hosts,
            core_delete_ruleset,
            core_read_active_config,
            core_retry_rulesets,
            core_update_rules,
            core_update_temp_rules,
            core_prune_expired_temp_rules,
            core_promote_temp_rule,
            core_set_connection_ask,
            core_network_policy_get,
            core_network_policy_set,
            core_update_geoip_db,
            core_auto_update_geoip_db,
            crate::adapters::update_release::check_release_update,
            core_decide_connection_ask,
            core_open_connection_ask_window,
            core_clash_proxy_delay_test,
            core_rules,
            core_clash_api_config,
            get_process_icon,
            core_resolve_process,
            core_close_connections,
            core_network_quality_test_start,
            core_network_quality_test_cancel,
            core_stun_test_start,
            core_stun_test_cancel,
            core_get_local_latencies,
            core_get_active_network_interface,
            core_get_local_network_info,
            core_get_external_ip_info,
            core_run_diagnostics,
            flush_dns_cache,
            test_dns_lookup
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application");

    app.run(move |app_handle, event| {
        match event {
            // Closing main quits fully — otherwise hidden connection-ask + tray keep process alive.
            #[cfg(target_os = "macos")]
            tauri::RunEvent::Reopen { .. } => {
                // Don't drag the main window up when the reopen was triggered by
                // interacting with the connection-ask popup (e.g. clicking Allow
                // while main is hidden to tray). Only a genuine dock-click reopen
                // with no popup on screen should restore main.
                let ask_visible = app_handle
                    .get_webview_window("connection-ask")
                    .and_then(|w| w.is_visible().ok())
                    .unwrap_or(false);
                if !ask_visible {
                    if let Some(win) = app_handle.get_webview_window("main") {
                        let _ = win.unminimize();
                        let _ = win.show();
                        let _ = win.set_focus();
                    }
                }
            }
            tauri::RunEvent::WindowEvent {
                label,
                event: tauri::WindowEvent::CloseRequested { api, .. },
                ..
            } => {
                if label == "main" {
                    let hide = app_handle
                        .try_state::<AppState>()
                        .map(|s| s.hide_on_close.load(std::sync::atomic::Ordering::Relaxed))
                        .unwrap_or(true);

                    if hide {
                        // Hide to tray instead of closing
                        api.prevent_close();
                        if let Some(win) = app_handle.get_webview_window("main") {
                            let _ = win.hide();
                        }
                    } else {
                        // Quit the entire app: prevent the native close to
                        // avoid reentrancy, then schedule exit on next tick.
                        api.prevent_close();
                        let h = app_handle.clone();
                        app_handle
                            .run_on_main_thread(move || {
                                h.exit(0);
                            })
                            .ok();
                    }
                } else if label == "connection-ask" {
                    // Keep webview warm; hide instead of destroy.
                    api.prevent_close();
                    if let Some(win) = app_handle.get_webview_window("connection-ask") {
                        let _ = win.hide();
                    }
                }
            }
            tauri::RunEvent::Exit => {
                if let Some(app_state) = app_handle.try_state::<AppState>() {
                    tauri::async_runtime::block_on(async {
                        let mut core = app_state.core.lock().await;
                        core.stop();
                    });
                }

                if let Some(state) = app_handle.try_state::<DaemonState>() {
                    if let Some(tx) = state.shutdown_tx.lock().unwrap().take() {
                        let _ = tx.send(());
                    }
                    if let Some(handle) = state.daemon_handle.lock().unwrap().take() {
                        let _ = tauri::async_runtime::block_on(handle);
                    }
                }

                if let Some(state) = app_handle.try_state::<NetworkTestState>() {
                    if let Some(handle) = state.task_handle.lock().unwrap().take() {
                        handle.abort();
                    }
                }

                if let Some(state) = app_handle.try_state::<StunTestState>() {
                    if let Some(handle) = state.task_handle.lock().unwrap().take() {
                        handle.abort();
                    }
                }

                if let Some(state) = app_handle.try_state::<AutoUpdateState>() {
                    if let Some(tx) = state.shutdown_tx.lock().unwrap().take() {
                        let _ = tx.send(());
                    }
                    if let Some(handle) = state.task_handle.lock().unwrap().take() {
                        handle.abort();
                    }
                }

                if let Some(state) = app_handle.try_state::<TempRuleGcState>() {
                    if let Some(tx) = state.shutdown_tx.lock().unwrap().take() {
                        let _ = tx.send(());
                    }
                    if let Some(handle) = state.task_handle.lock().unwrap().take() {
                        handle.abort();
                    }
                }
            }
            _ => {}
        }
    });
}
