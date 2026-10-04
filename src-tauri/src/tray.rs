//! macOS menu-bar tray icon for Zeytun.
//!
//! Builds a native `TrayIcon` with a live `↑ x ↓ y` title and a menu to show the
//! window and toggle outbound mode / system proxy / TUN. Menu handlers never hold
//! the core `MutexGuard` across an `.await`: they clone the `Arc<Mutex<CoreManager>>`
//! and run the blocking core call inside `spawn_blocking` + `blocking_lock`.

use std::collections::HashMap;
use tauri::{
    menu::{
        CheckMenuItem, CheckMenuItemBuilder, Menu, MenuBuilder, MenuItemBuilder, SubmenuBuilder,
    },
    tray::TrayIconBuilder,
    AppHandle, Emitter, Manager, Wry,
};

use crate::core::dto::{InboundMode, OutboundMode, ZeytunCore};

pub const TRAY_ID: &str = "zeytun-tray";

// Menu item ids — matched against `MenuEvent::id()`.
const MI_SHOW: &str = "tray-show";
const MI_INSPECTOR: &str = "tray-inspector";
const MI_MODE_GLOBAL: &str = "tray-mode-global";
const MI_MODE_RULE: &str = "tray-mode-rule";
const MI_MODE_DIRECT: &str = "tray-mode-direct";
const MI_SYSTEM_PROXY: &str = "tray-system-proxy";
const MI_TUN: &str = "tray-tun";
const MI_QUIT: &str = "tray-quit";

/// Snapshot of the toggle state used to seed the menu checkmarks at build time
/// and to push reverse-sync updates from the frontend. `Copy` so it can be moved
/// into the main-thread closure without holding the core guard.
#[derive(Clone, Copy)]
pub struct TrayState {
    pub mode: OutboundMode,
    pub system_proxy: bool,
    pub tun: bool,
}

impl TrayState {
    /// Extract the tray-relevant toggles from the full profile. Caller holds the
    /// core guard only for this read; drop it before touching the tray.
    pub fn from_profile(profile: &ZeytunCore) -> Self {
        Self {
            mode: profile.outbound_mode,
            system_proxy: profile.local_proxy.system_proxy.unwrap_or(false),
            tun: matches!(profile.local_proxy.mode, Some(InboundMode::Tun)),
        }
    }
}

/// Holds the tray menu and direct references to the check items so we don't
/// have to rely on `Menu::get` which does not recursively search submenus.
struct TrayMenu {
    menu: Menu<Wry>,
    checks: HashMap<&'static str, CheckMenuItem<Wry>>,
}

/// Build the tray icon + menu and attach it to the app. Call once in `setup`.
pub fn build_tray(app: &AppHandle, initial: &TrayState) -> tauri::Result<()> {
    let show = MenuItemBuilder::with_id(MI_SHOW, "Show Main Menu").build(app)?;
    let inspector = MenuItemBuilder::with_id(MI_INSPECTOR, "Open Traffic Monitor").build(app)?;

    let mode_global = CheckMenuItemBuilder::with_id(MI_MODE_GLOBAL, "Global Proxy")
        .checked(matches!(initial.mode, OutboundMode::Global))
        .build(app)?;
    let mode_rule = CheckMenuItemBuilder::with_id(MI_MODE_RULE, "Rule-Based Proxy")
        .checked(matches!(initial.mode, OutboundMode::Rule))
        .build(app)?;
    let mode_direct = CheckMenuItemBuilder::with_id(MI_MODE_DIRECT, "Direct Outbound")
        .checked(matches!(initial.mode, OutboundMode::Direct))
        .build(app)?;

    let mode_submenu = SubmenuBuilder::new(app, "Outbound Mode")
        .item(&mode_global)
        .item(&mode_rule)
        .item(&mode_direct)
        .build()?;

    let system_proxy = CheckMenuItemBuilder::with_id(MI_SYSTEM_PROXY, "Set as System Proxy")
        .checked(initial.system_proxy)
        .build(app)?;
    let tun = CheckMenuItemBuilder::with_id(MI_TUN, "Turn on Tun Mode")
        .checked(initial.tun)
        .build(app)?;

    let quit = MenuItemBuilder::with_id(MI_QUIT, "Quit").build(app)?;

    let menu = MenuBuilder::new(app)
        .item(&show)
        .item(&inspector)
        .separator()
        .item(&mode_submenu)
        .separator()
        .item(&system_proxy)
        .item(&tun)
        .separator()
        .item(&quit)
        .build()?;

    // Keep direct references to the check items so we can easily mutate them.
    let mut checks = HashMap::new();
    checks.insert(MI_MODE_GLOBAL, mode_global);
    checks.insert(MI_MODE_RULE, mode_rule);
    checks.insert(MI_MODE_DIRECT, mode_direct);
    checks.insert(MI_SYSTEM_PROXY, system_proxy);
    checks.insert(MI_TUN, tun);

    app.manage(TrayMenu {
        menu: menu.clone(),
        checks,
    });

    TrayIconBuilder::with_id(TRAY_ID)
        .icon(
            app.default_window_icon()
                .cloned()
                .ok_or_else(|| tauri::Error::AssetNotFound("default window icon".to_string()))?,
        )
        // No text title; we render text natively inside the icon.
        .menu(&menu)
        .on_menu_event(handle_menu_event)
        .build(app)?;

    // Draw the initial 0 KB/s icon immediately
    crate::tray_icon::update_tray_icon(app, 0, 0);

    Ok(())
}

fn handle_menu_event(app: &AppHandle, event: tauri::menu::MenuEvent) {
    match event.id().as_ref() {
        MI_SHOW => {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.set_focus();
            }
        }
        MI_INSPECTOR => open_inspector(app),
        MI_MODE_GLOBAL => set_mode(app, OutboundMode::Global),
        MI_MODE_RULE => set_mode(app, OutboundMode::Rule),
        MI_MODE_DIRECT => set_mode(app, OutboundMode::Direct),
        MI_SYSTEM_PROXY => toggle_system_proxy(app),
        MI_TUN => toggle_tun(app),
        MI_QUIT => app.exit(0),
        _ => {}
    }
}

/// Open the Inspector route in the main window. Navigation itself is handled by
/// the frontend (`open-inspector` event → `goto("/inspector")`) so the app URL
/// never has to be resolved from Rust (dev vs prod protocols differ).
fn open_inspector(app: &AppHandle) {
    if let Some(win) = app.get_webview_window("main") {
        let _ = win.show();
        let _ = win.set_focus();
    }
    let _ = app.emit("open-inspector", ());
}

/// Run a blocking core mutation off the async runtime without holding the guard
/// across any await. Mirrors `with_core_blocking` in lib.rs. On success emits
/// `profile-updated` so the frontend re-syncs its reactive state.
fn with_core<F>(app: &AppHandle, f: F)
where
    F: FnOnce(&mut crate::core::manager::CoreManager) -> Result<(), crate::error::CommandError>
        + Send
        + 'static,
{
    let Some(state) = app.try_state::<crate::AppState>() else {
        return;
    };
    let core = state.core_arc();
    let app = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let mut guard = core.blocking_lock();
        let result = f(&mut guard);
        drop(guard); // release the lock before emitting
        match result {
            Ok(()) => {
                use tauri::Emitter;
                let _ = app.emit("profile-updated", ());
            }
            Err(e) => eprintln!("[Tray] core call failed: {e}"),
        }
    });
}

fn set_mode(app: &AppHandle, mode: OutboundMode) {
    let handle = app.clone();
    let _ = app.run_on_main_thread(move || {
        sync_mode_checks(&handle, mode);
    });
    with_core(app, move |core| core.set_mode(mode));
}

fn toggle_system_proxy(app: &AppHandle) {
    let enabled = check_state(app, MI_SYSTEM_PROXY);
    with_core(app, move |core| core.set_system_proxy(enabled));
}

fn toggle_tun(app: &AppHandle) {
    // Tauri flips the checkmark before the handler runs; read the new state and
    // map it to the inbound mode ("tun" when on, "mixed" when off).
    let on = check_state(app, MI_TUN);
    let mode = if on { "tun" } else { "mixed" };
    with_core(app, move |core| core.set_inbound_mode(mode));
}

/// Read the current checked state of a check menu item by id.
fn check_state(app: &AppHandle, id: &str) -> bool {
    let Some(tray_state) = app.try_state::<TrayMenu>() else {
        return false;
    };
    tray_state
        .checks
        .get(id)
        .and_then(|c| c.is_checked().ok())
        .unwrap_or(false)
}

/// Emulate radio behaviour: check the selected mode, uncheck the other two,
/// and re-attach the menu to the tray to force macOS to immediately re-render.
fn sync_mode_checks(app: &AppHandle, mode: OutboundMode) {
    let Some(tray_state) = app.try_state::<TrayMenu>() else {
        return;
    };
    for (id, this) in [
        (MI_MODE_GLOBAL, OutboundMode::Global),
        (MI_MODE_RULE, OutboundMode::Rule),
        (MI_MODE_DIRECT, OutboundMode::Direct),
    ] {
        if let Some(check) = tray_state.checks.get(id) {
            let _ = check.set_checked(this == mode);
        }
    }

    // Force macOS to immediately visually repaint the open menu
    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        let _ = tray.set_menu(Some(tray_state.menu.clone()));
    }
}

/// Set a single check menu item by id, and force a repaint.
fn set_check(app: &AppHandle, id: &str, checked: bool) {
    let Some(tray_state) = app.try_state::<TrayMenu>() else {
        return;
    };
    if let Some(check) = tray_state.checks.get(id) {
        let _ = check.set_checked(checked);
    }

    // Force macOS to immediately visually repaint the open menu
    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        let _ = tray.set_menu(Some(tray_state.menu.clone()));
    }
}

/// Reverse sync: push the current core toggle state onto the tray checkmarks.
/// Called from Tauri commands after a frontend-initiated mutation. macOS menu
/// items are main-thread-affine, and commands run off the main thread, so the
/// update is dispatched via `run_on_main_thread`. Takes a `Copy` snapshot — no
/// core guard is held across this call.
pub fn sync_tray_ui(app: &AppHandle, state: TrayState) {
    let handle = app.clone();
    let _ = app.run_on_main_thread(move || {
        sync_mode_checks(&handle, state.mode);
        set_check(&handle, MI_SYSTEM_PROXY, state.system_proxy);
        set_check(&handle, MI_TUN, state.tun);
    });
}

/// Update the tray title with the live aggregate up/down rate. Allocates only the
/// final title string; called once per traffic tick.
pub fn set_traffic_title(app: &AppHandle, up_bps: i64, down_bps: i64) {
    crate::tray_icon::update_tray_icon(app, up_bps, down_bps);
}
