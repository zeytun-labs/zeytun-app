//! Native tray menu synchronized with the core, including failed operations.

use crate::core::dto::{InboundMode, OutboundMode, RuntimePhase, DEFAULT_SELECTOR_POLICY_TAG};
use crate::core::manager::CoreManager;
use crate::tray_icon::TrafficLabels;
use std::collections::HashMap;
use std::sync::Mutex;
use tauri::{
    menu::{
        CheckMenuItem, CheckMenuItemBuilder, Menu, MenuBuilder, MenuItem, MenuItemBuilder, Submenu,
        SubmenuBuilder,
    },
    tray::TrayIconBuilder,
    AppHandle, Emitter, Manager, Wry,
};

pub const TRAY_ID: &str = "zeytun-tray";
const MI_SHOW: &str = "tray-show";
const MI_INSPECTOR: &str = "tray-inspector";
const MI_MODE_GLOBAL: &str = "tray-mode-global";
const MI_MODE_RULE: &str = "tray-mode-rule";
const MI_MODE_DIRECT: &str = "tray-mode-direct";
const MI_SYSTEM_PROXY: &str = "tray-system-proxy";
const MI_TUN: &str = "tray-tun";
const MI_CORE: &str = "tray-core";
const MI_RESTART: &str = "tray-restart";
const MI_QUIT: &str = "tray-quit";
const RESTART_INDEX: usize = 10;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TrayState {
    mode: OutboundMode,
    system_proxy: bool,
    tun: bool,
    phase: RuntimePhase,
    pending_apply: bool,
    profile_name: String,
    selected_name: String,
}

impl TrayState {
    /// Read a complete snapshot while the caller owns the core guard.
    pub fn from_core(core: &mut CoreManager) -> Self {
        let runtime = core.status();
        let profile = core.profile_get();
        let profile_name = core
            .profile_list()
            .ok()
            .and_then(|profiles| profiles.into_iter().find(|p| p.id == profile.id))
            .map(|p| p.name)
            .unwrap_or_else(|| profile.id.clone());
        let selected = profile
            .policies
            .iter()
            .find(|p| p.tag == DEFAULT_SELECTOR_POLICY_TAG)
            .and_then(|p| p.selected_member_tag.as_deref());
        let selected_name = if profile.outbound_mode == OutboundMode::Direct {
            "Direct".into()
        } else {
            selected
                .map(|tag| {
                    profile
                        .proxies
                        .iter()
                        .find(|p| p.tag == tag)
                        .map(|p| p.title.clone())
                        .or_else(|| {
                            profile
                                .policies
                                .iter()
                                .find(|p| p.tag == tag)
                                .map(|p| p.name.clone())
                        })
                        .unwrap_or_else(|| tag.to_string())
                })
                .unwrap_or_else(|| "Automatic".into())
        };
        Self {
            mode: profile.outbound_mode,
            system_proxy: profile.local_proxy.system_proxy.unwrap_or(false),
            tun: matches!(profile.local_proxy.mode, Some(InboundMode::Tun)),
            phase: runtime.phase,
            pending_apply: runtime.pending_apply,
            profile_name,
            selected_name,
        }
    }

    fn is_running(&self) -> bool {
        self.phase == RuntimePhase::Running
    }
    fn is_transitioning(&self) -> bool {
        matches!(self.phase, RuntimePhase::Starting | RuntimePhase::Stopping)
    }
    fn needs_restart(&self) -> bool {
        self.is_running() && self.pending_apply
    }
    fn status_text(&self) -> &'static str {
        match self.phase {
            RuntimePhase::Idle => "Core: Stopped",
            RuntimePhase::Starting => "Core: Starting…",
            RuntimePhase::Running => "Core: Running",
            RuntimePhase::Stopping => "Core: Stopping…",
            RuntimePhase::Error => "Core: Error",
        }
    }
    fn mode_text(&self) -> String {
        let mode = match self.mode {
            OutboundMode::Global => "Global",
            OutboundMode::Rule => "Rule",
            OutboundMode::Direct => "Direct",
        };
        format!("Outbound Mode: {mode}")
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct TrayModel {
    state: TrayState,
    traffic: TrafficLabels,
    busy: Option<&'static str>,
    last_error: Option<String>,
}

impl TrayModel {
    fn complete_action(&mut self, state: TrayState, error: Option<String>) {
        self.state = state;
        self.busy = None;
        self.last_error = error;
    }

    fn tooltip(&self) -> String {
        let mut lines = vec![
            "Zeytun".to_string(),
            self.traffic.tooltip.clone(),
            self.state.status_text().to_string(),
            self.state.mode_text(),
            format!("Profile: {}", self.state.profile_name),
            format!("Selected: {}", self.state.selected_name),
        ];
        if self.state.needs_restart() {
            lines.push("Restart required to apply changes".into());
        }
        if let Some(busy) = self.busy {
            lines.push(format!("{busy}…"));
        }
        if let Some(error) = &self.last_error {
            lines.push(format!("Last action failed: {error}"));
        }
        let tooltip = lines.join("\n");
        // Windows reserves 128 UTF-16 code units including the terminator.
        // Keep rates near the start so long profile/proxy names cannot hide them.
        #[cfg(target_os = "windows")]
        if tooltip.encode_utf16().count() > 127 {
            let mut length = 0;
            let mut short = tooltip
                .chars()
                .take_while(|c| {
                    length += c.len_utf16();
                    length <= 126
                })
                .collect::<String>();
            short.push('…');
            return short;
        }
        tooltip
    }
}

struct TrayMenu {
    menu: Menu<Wry>,
    checks: HashMap<&'static str, CheckMenuItem<Wry>>,
    status: MenuItem<Wry>,
    mode: Submenu<Wry>,
    core_control: MenuItem<Wry>,
    restart: MenuItem<Wry>,
    model: Mutex<TrayModel>,
}

pub fn build_tray(app: &AppHandle, initial: &TrayState) -> tauri::Result<()> {
    let status = MenuItemBuilder::with_id("tray-status", initial.status_text())
        .enabled(false)
        .build(app)?;
    let show = MenuItemBuilder::with_id(MI_SHOW, "Open Zeytun").build(app)?;
    let inspector = MenuItemBuilder::with_id(MI_INSPECTOR, "Traffic Monitor").build(app)?;
    let mode_global = CheckMenuItemBuilder::with_id(MI_MODE_GLOBAL, "Global")
        .checked(initial.mode == OutboundMode::Global)
        .build(app)?;
    let mode_rule = CheckMenuItemBuilder::with_id(MI_MODE_RULE, "Rule")
        .checked(initial.mode == OutboundMode::Rule)
        .build(app)?;
    let mode_direct = CheckMenuItemBuilder::with_id(MI_MODE_DIRECT, "Direct")
        .checked(initial.mode == OutboundMode::Direct)
        .build(app)?;
    let mode = SubmenuBuilder::new(app, initial.mode_text())
        .item(&mode_global)
        .item(&mode_rule)
        .item(&mode_direct)
        .build()?;
    let system_proxy = CheckMenuItemBuilder::with_id(MI_SYSTEM_PROXY, "System Proxy")
        .checked(initial.system_proxy)
        .build(app)?;
    let tun = CheckMenuItemBuilder::with_id(MI_TUN, "TUN Mode")
        .checked(initial.tun)
        .build(app)?;
    let core_control = MenuItemBuilder::with_id(
        MI_CORE,
        if initial.is_running() {
            "Stop Core"
        } else {
            "Start Core"
        },
    )
    .build(app)?;
    let restart = MenuItemBuilder::with_id(MI_RESTART, "Restart Core").build(app)?;
    let quit = MenuItemBuilder::with_id(MI_QUIT, "Quit Zeytun")
        .accelerator("CmdOrCtrl+Q")
        .build(app)?;
    let menu = MenuBuilder::new(app)
        .item(&status)
        .separator()
        .item(&show)
        .item(&inspector)
        .separator()
        .item(&mode)
        .item(&system_proxy)
        .item(&tun)
        .separator()
        .item(&core_control)
        .separator()
        .item(&quit)
        .build()?;
    if initial.needs_restart() {
        menu.insert(&restart, RESTART_INDEX)?;
    }
    let checks = HashMap::from([
        (MI_MODE_GLOBAL, mode_global),
        (MI_MODE_RULE, mode_rule),
        (MI_MODE_DIRECT, mode_direct),
        (MI_SYSTEM_PROXY, system_proxy),
        (MI_TUN, tun),
    ]);
    let model = TrayModel {
        state: initial.clone(),
        traffic: TrafficLabels::from_rates(0, 0),
        busy: None,
        last_error: None,
    };
    let tooltip = model.tooltip();
    app.manage(TrayMenu {
        menu: menu.clone(),
        checks,
        status,
        mode,
        core_control,
        restart,
        model: Mutex::new(model),
    });
    TrayIconBuilder::with_id(TRAY_ID)
        .icon(crate::tray_icon::initial_icon(app)?)
        .tooltip(tooltip)
        .menu(&menu)
        .on_menu_event(handle_menu_event)
        .build(app)?;
    crate::tray_icon::initialize(app)?;
    schedule_refresh(app);
    Ok(())
}

fn show_main(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}

fn handle_menu_event(app: &AppHandle, event: tauri::menu::MenuEvent) {
    match event.id().as_ref() {
        MI_SHOW => show_main(app),
        MI_INSPECTOR => {
            show_main(app);
            let _ = app.emit("open-inspector", ());
        }
        MI_MODE_GLOBAL => run_action(app, TrayAction::Mode(OutboundMode::Global)),
        MI_MODE_RULE => run_action(app, TrayAction::Mode(OutboundMode::Rule)),
        MI_MODE_DIRECT => run_action(app, TrayAction::Mode(OutboundMode::Direct)),
        MI_SYSTEM_PROXY => run_action(app, TrayAction::SystemProxy),
        MI_TUN => run_action(app, TrayAction::Tun),
        MI_CORE => run_action(app, TrayAction::ToggleCore),
        MI_RESTART => run_action(app, TrayAction::Restart),
        MI_QUIT => app.exit(0),
        _ => {}
    }
}

#[derive(Clone, Copy)]
enum TrayAction {
    Mode(OutboundMode),
    SystemProxy,
    Tun,
    ToggleCore,
    Restart,
}

impl TrayAction {
    fn title(self) -> &'static str {
        match self {
            Self::Mode(_) => "Changing outbound mode",
            Self::SystemProxy => "Changing system proxy",
            Self::Tun => "Changing TUN mode",
            Self::ToggleCore => "Updating core",
            Self::Restart => "Restarting core",
        }
    }
    fn apply(self, core: &mut CoreManager) -> Result<(), crate::error::CommandError> {
        match self {
            Self::Mode(mode) => core.set_mode(mode),
            // The backend is authoritative; never derive intent from native
            // checkmarks, which the platform may have already flipped.
            Self::SystemProxy => {
                core.set_system_proxy(!core.profile_get().local_proxy.system_proxy.unwrap_or(false))
            }
            Self::Tun => {
                let on = matches!(core.profile_get().local_proxy.mode, Some(InboundMode::Tun));
                core.set_inbound_mode(if on { "mixed" } else { "tun" })
            }
            Self::ToggleCore => {
                if core.is_running() {
                    core.stop();
                } else {
                    core.start()?;
                    select_global_root(core);
                }
                Ok(())
            }
            Self::Restart => {
                let state = TrayState::from_core(core);
                if state.needs_restart() {
                    core.restart()?;
                    select_global_root(core);
                }
                Ok(())
            }
        }
    }
}

/// Match the regular Start command after the controller becomes ready. The
/// built-in GLOBAL group must route through the active profile's root policy.
fn select_global_root(core: &CoreManager) {
    if core.profile_get().outbound_mode != OutboundMode::Global {
        return;
    }
    if let Some(api) = core.clash_api_client() {
        for _ in 0..crate::core::constants::CLASH_API_SELECT_RETRY_COUNT {
            if api
                .select_proxy("GLOBAL", DEFAULT_SELECTOR_POLICY_TAG)
                .is_ok()
            {
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(
                crate::core::constants::CLASH_API_SELECT_RETRY_DELAY_MS,
            ));
        }
        crate::clog!("[Tray] could not restore GLOBAL root-policy selection after start");
    }
}

fn run_action(app: &AppHandle, action: TrayAction) {
    let Some(app_state) = app.try_state::<crate::AppState>() else {
        return;
    };
    let Some(menu) = app.try_state::<TrayMenu>() else {
        return;
    };
    let Ok(mut model) = menu.model.lock() else {
        return;
    };
    if model.busy.is_some() || model.state.is_transitioning() {
        drop(model);
        // Undo any native checkbox flip from an ignored extra click.
        schedule_refresh(app);
        return;
    }
    model.busy = Some(action.title());
    model.last_error = None;
    drop(model);
    schedule_refresh(app);
    let core = app_state.core_arc();
    let app = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let mut guard = core.blocking_lock();
        let result = action.apply(&mut guard);
        // Capture even on failure: TUN may have rolled back while native checks
        // still show the attempted value; runtime state may have changed too.
        let snapshot = TrayState::from_core(&mut guard);
        if let Some(menu) = app.try_state::<TrayMenu>() {
            if let Ok(mut model) = menu.model.lock() {
                model.complete_action(snapshot, result.as_ref().err().map(ToString::to_string));
            }
        }
        drop(guard);
        schedule_refresh(&app);
        let _ = app.emit("profile-updated", ());
        if let Err(error) = result {
            let message = error.to_string();
            if let Some(events) = app.try_state::<std::sync::Arc<crate::core::events::EventBus>>() {
                events.error("Tray action failed", &message);
            }
            let _ = app.emit("core-notification", serde_json::json!({
                "id": uuid::Uuid::new_v4().to_string(),
                "ts_ms": std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or_default(),
                "scope": 7, "code": "TRAY_ACTION_FAILED", "severity": 2,
                "title": "Tray action failed", "message": message, "attrs": {}
            }));
        }
    });
}

/// Main-thread jobs read the latest model rather than a captured stale snapshot.
fn schedule_refresh(app: &AppHandle) {
    let handle = app.clone();
    if let Err(error) = app.run_on_main_thread(move || apply_model(&handle)) {
        crate::clog!("[Tray] could not schedule update: {error}");
    }
}

fn apply_model(app: &AppHandle) {
    let Some(menu) = app.try_state::<TrayMenu>() else {
        return;
    };
    let Ok(model) = menu.model.lock().map(|model| model.clone()) else {
        return;
    };
    let enabled = model.busy.is_none() && !model.state.is_transitioning();
    let status = if let Some(action) = model.busy {
        format!("{action}…")
    } else if model.last_error.is_some() {
        format!("{} — Action Failed", model.state.status_text())
    } else {
        model.state.status_text().into()
    };
    let result = (|| -> tauri::Result<()> {
        menu.status.set_text(status)?;
        menu.mode.set_text(model.state.mode_text())?;
        menu.mode.set_enabled(enabled)?;
        for (id, checked) in [
            (MI_MODE_GLOBAL, model.state.mode == OutboundMode::Global),
            (MI_MODE_RULE, model.state.mode == OutboundMode::Rule),
            (MI_MODE_DIRECT, model.state.mode == OutboundMode::Direct),
            (MI_SYSTEM_PROXY, model.state.system_proxy),
            (MI_TUN, model.state.tun),
        ] {
            if let Some(check) = menu.checks.get(id) {
                check.set_checked(checked)?;
                check.set_enabled(enabled)?;
            }
        }
        menu.core_control.set_text(if model.state.is_running() {
            "Stop Core"
        } else {
            "Start Core"
        })?;
        menu.core_control.set_enabled(enabled)?;
        let restart_present = menu.menu.get(MI_RESTART).is_some();
        if model.state.needs_restart() && !restart_present {
            menu.menu.insert(&menu.restart, RESTART_INDEX)?;
        } else if !model.state.needs_restart() && restart_present {
            menu.menu.remove(&menu.restart)?;
        }
        menu.restart.set_enabled(enabled)?;
        if let Some(tray) = app.tray_by_id(TRAY_ID) {
            tray.set_tooltip(Some(model.tooltip()))?;
        }
        crate::tray_icon::update_traffic(&model.traffic);
        Ok(())
    })();
    if let Err(error) = result {
        crate::clog!("[Tray] could not update menu: {error}");
    }
}

pub fn sync_tray_ui(app: &AppHandle, state: TrayState) {
    let Some(menu) = app.try_state::<TrayMenu>() else {
        return;
    };
    let changed = if let Ok(mut model) = menu.model.lock() {
        if model.state == state {
            false
        } else {
            model.state = state;
            true
        }
    } else {
        false
    };
    if changed {
        schedule_refresh(app);
    }
}

/// Called once per traffic tick, including while the main window is hidden.
/// A busy backend is skipped; the next tick catches pending revisions and exits.
pub fn set_traffic_title(app: &AppHandle, up_bps: i64, down_bps: i64) {
    if let Some(state) = app.try_state::<crate::AppState>() {
        let core = state.core_arc();
        if let Ok(mut guard) = core.try_lock() {
            sync_tray_ui(app, TrayState::from_core(&mut guard));
        };
    }
    let Some(menu) = app.try_state::<TrayMenu>() else {
        return;
    };
    let traffic = TrafficLabels::from_rates(up_bps, down_bps);
    let changed = if let Ok(mut model) = menu.model.lock() {
        if model.traffic == traffic {
            false
        } else {
            model.traffic = traffic;
            true
        }
    } else {
        false
    };
    if changed {
        schedule_refresh(app);
    }
}
