// SettingsDraft — owns ALL settings state & apply logic for the settings dialog.
// The dialog/sections are a dumb view over this class: bind to fields, call
// apply()/reset(). No IPC or store writes happen outside apply().

import { toast } from "svelte-sonner";
import { enable, disable } from "@tauri-apps/plugin-autostart";
import {
  settingsStore,
  defaultSettings,
  type Settings,
} from "$lib/stores/settings.svelte";
import { profileStore } from "$lib/stores/profile.svelte";
import {
  coreSetConnectionAsk,
  coreSetListenPort,
  coreSetLogLevel,
  coreSetHideOnClose,
  coreNetworkPolicyGet,
  coreNetworkPolicySet,
  type NetworkPolicy,
  type TrafficKind,
} from "$lib/core/api";
import { themePreference, type Theme } from "$lib/theme.svelte";
import { errorMessage } from "$lib/errors";
import {
  isValidListenPort,
  validateConnectivity,
  type SettingsField,
} from "./validation";

const FALLBACK_PORT = "6060";

export type LogLevel = "trace" | "debug" | "info" | "warn" | "error" | "fatal" | "panic";

/**
 * Select value for a traffic route: `direct` or `local_proxy`.
 *
 * Only those two exist because the app cannot dial a policy tag itself — a named
 * policy is carried by the local mixed inbound, so offering one would present a
 * choice that resolves to the same wire behaviour as "Through Zeytun".
 */
export type RouteValue = string;

export const TRAFFIC_KINDS: TrafficKind[] = ["app_update", "geoip", "subscription"];

export function encodeRoute(policy: NetworkPolicy | undefined): RouteValue {
  if (!policy) return "direct";
  // A stored named policy (older row) degrades to the local proxy at runtime, so
  // it reads back as "Through Zeytun" rather than as a value with no menu item.
  return policy.kind === "direct" ? "direct" : "local_proxy";
}

export function decodeRoute(value: RouteValue): NetworkPolicy {
  return value === "local_proxy" ? { kind: "local_proxy" } : { kind: "direct" };
}

function emptyRoutes(): Record<TrafficKind, RouteValue> {
  return { app_update: "direct", geoip: "direct", subscription: "direct" };
}

function currentListenPort(): string {
  return (
    profileStore.profile?.local_proxy.mixed_port?.toString() ?? FALLBACK_PORT
  );
}

/** The connection-ask source of truth is the profile; the store is a cache. */
function activeConnectionAsk(): boolean {
  return (
    settingsStore.settings.connectionAskEnabled ??
    profileStore.profile?.local_proxy.connection_ask?.enabled ??
    false
  );
}

/** Run one side-effect step; toast + log on failure instead of dying silently. */
async function step(action: () => Promise<unknown>, label: string): Promise<boolean> {
  try {
    await action();
    return true;
  } catch (err) {
    toast.error(`Failed to update ${label}`, { description: errorMessage(err) });
    console.error(`settings: ${label} update failed`, err);
    return false;
  }
}

export class SettingsDraft {
  settings = $state<Settings>({ ...settingsStore.settings });
  listenPort = $state(currentListenPort());
  theme = $state<Theme>(themePreference.value);
  errors = $state<Partial<Record<SettingsField, string>>>({});
  saving = $state(false);
  /** Draft route per traffic class, as a flat Select value. */
  routes = $state<Record<TrafficKind, RouteValue>>(emptyRoutes());
  /** Last known persisted routes — the baseline `hasChanges` compares against. */
  savedRoutes = $state<Record<TrafficKind, RouteValue>>(emptyRoutes());

  hasChanges = $derived.by(() => {
    if (this.listenPort !== currentListenPort()) return true;
    if (this.theme !== themePreference.value) return true;
    if ((this.settings.connectionAskEnabled ?? false) !== activeConnectionAsk()) {
      return true;
    }
    for (const traffic of TRAFFIC_KINDS) {
      if (this.routes[traffic] !== this.savedRoutes[traffic]) return true;
    }

    for (const key of Object.keys(this.settings) as (keyof Settings)[]) {
      if (key === "connectionAskEnabled" || key === "receivePrereleaseUpdates") continue; // normalized above / applied immediately
      if (this.settings[key] !== settingsStore.settings[key]) return true;
    }
    return false;
  });

  // NOTE: $effect is not allowed in .svelte.ts modules, only in .svelte
  // component contexts — keep reactive glue in the dialog component.

  /**
   * Read the stored routes. Absent classes are Direct, so a fresh install with
   * no rows still lands on a coherent state. Failures leave the defaults in
   * place — the dialog must stay usable when the backend is unreachable.
   */
  async loadRoutes() {
    try {
      const stored = await coreNetworkPolicyGet();
      const next = emptyRoutes();
      for (const traffic of TRAFFIC_KINDS) {
        next[traffic] = encodeRoute(stored[traffic]);
      }
      this.routes = { ...next };
      this.savedRoutes = { ...next };
    } catch (err) {
      console.error("settings: failed to read network routes", err);
    }
  }

  /** Re-sync drafts from live sources (store + profile) without clearing touched. */
  sync() {
    this.listenPort = currentListenPort();
    this.settings = { ...settingsStore.settings };
    this.settings.connectionAskEnabled = activeConnectionAsk();
    this.theme = themePreference.value;
    this.errors = {};
    // Async: the DB read cannot block the synchronous dialog-open path.
    void this.loadRoutes();
  }

  /** Full reset — used on dialog open and cancel. */
  reset() {
    this.sync();
  }

  resetConnectivity() {
    this.settings.internetTestUrl = defaultSettings.internetTestUrl;
    this.settings.proxyTestUrl = defaultSettings.proxyTestUrl;
    this.settings.networkConfigUrl = defaultSettings.networkConfigUrl;
    this.settings.stunServer = defaultSettings.stunServer;
    this.errors = {};
  }

  /**
   * Apply all pending draft changes. Returns false (and toasts) when
   * validation fails — callers must not close the dialog in that case.
   */
  async apply(): Promise<boolean> {
    const errors: Partial<Record<SettingsField, string>> = validateConnectivity({
      internetTestUrl: this.settings.internetTestUrl,
      proxyTestUrl: this.settings.proxyTestUrl,
      networkConfigUrl: this.settings.networkConfigUrl,
      stunServer: this.settings.stunServer,
    });
    // Validated here rather than mid-apply so the port shows an inline error
    // like every other field, instead of a toast after some writes landed.
    if (
      this.listenPort !== currentListenPort() &&
      !isValidListenPort(this.listenPort)
    ) {
      errors.listenPort = "Enter a port between 1 and 65535";
    }
    this.errors = errors;
    if (Object.keys(errors).length > 0) return false;

    this.saving = true;

    try {
      // --- Listen port (lives on the profile, not the settings store) ---
      if (this.listenPort !== currentListenPort()) {
        if (await step(() => coreSetListenPort(parseInt(this.listenPort, 10)), "listen port")) {
          // Set pending restart flag instead of auto-refreshing
          profileStore.pendingRestart = true;
        }
      }

      // --- Launch at login (real OS state via tauri-plugin-autostart) ---
      if (this.settings.launchAtLogin !== settingsStore.settings.launchAtLogin) {
        await step(
          () => (this.settings.launchAtLogin ? enable() : disable()),
          "launch at login",
        );
      }

      // --- Hide on Close ---
      if (this.settings.hideOnClose !== settingsStore.settings.hideOnClose) {
        await step(() => coreSetHideOnClose(this.settings.hideOnClose), "hide on close");
      }

      // --- Log level ---
      if (this.settings.logLevel !== settingsStore.settings.logLevel) {
        if (await step(() => coreSetLogLevel(this.settings.logLevel), "log level")) {
          // Set pending restart flag instead of auto-refreshing
          profileStore.pendingRestart = true;
        }
      }

      // --- Theme (kept in the draft like everything else; applied on save) ---
      if (this.theme !== themePreference.value) {
        themePreference.set(this.theme);
      }

      // --- DNS (fake_ip lives on the active profile) ---
      if (
        this.settings.enableFakeIp !== (profileStore.profile?.dns?.fake_ip ?? false)
      ) {
        const profile = profileStore.profile;
        if (profile) {
          const ok = await step(
            () =>
              profileStore.updateProfile({
                id: profile.id,
                dns: {
                  ...profile.dns,
                  fake_ip: this.settings.enableFakeIp,
                },
              }),
            "DNS settings",
          );
          if (!ok) return false;
          profileStore.pendingRestart = true;
        }
      }

      // --- Connection ask (merge with existing config — never clobber it) ---
      if ((this.settings.connectionAskEnabled ?? false) !== activeConnectionAsk()) {
        const existing = profileStore.profile?.local_proxy.connection_ask;
        await step(
          () =>
            coreSetConnectionAsk({
              enabled: this.settings.connectionAskEnabled ?? false,
              timeout_ms: existing?.timeout_ms ?? 60000,
              group_by: existing?.group_by ?? "process",
              remember_default: existing?.remember_default ?? true,
            }),
          "connection ask",
        );
        // Set pending restart flag instead of auto-refreshing
        profileStore.pendingRestart = true;
      }

      // --- Network routes for the app's own traffic (one row per changed class) ---
      for (const traffic of TRAFFIC_KINDS) {
        if (this.routes[traffic] === this.savedRoutes[traffic]) continue;
        const value = this.routes[traffic];
        const ok = await step(
          () => coreNetworkPolicySet(traffic, decodeRoute(value)),
          "network route",
        );
        // Only advance the baseline on success, so a failed write stays visible
        // as a pending change instead of silently reverting in the UI.
        if (ok) this.savedRoutes[traffic] = value;
      }
    } finally {
      this.saving = false;
    }

    // Persist the localStorage-backed subset; the rest went to the backend above.
    settingsStore.settings = {
      ...this.settings,
      receivePrereleaseUpdates: settingsStore.settings.receivePrereleaseUpdates,
    };
    settingsStore.save();

    return true;
  }
}
