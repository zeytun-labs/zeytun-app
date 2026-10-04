import { enable, isEnabled, disable } from '@tauri-apps/plugin-autostart';
import { coreSetHideOnClose } from '$lib/core/api';

export const SETTINGS_STORAGE_KEY = "zeytun-settings";

export interface Settings {
  launchAtLogin: boolean;
  autoUpdateGeoIp: boolean;
  receivePrereleaseUpdates: boolean;
  internetTestUrl: string;
  proxyTestUrl: string;
  networkConfigUrl: string;
  stunServer: string;
  logLevel: string;
  enableFakeIp: boolean;
  connectionAskEnabled: boolean;
  hideOnClose: boolean;
}

export const defaultSettings: Settings = {
  launchAtLogin: false,
  autoUpdateGeoIp: true,
  receivePrereleaseUpdates: false,
  internetTestUrl: "google.com",
  proxyTestUrl: "http://cp.cloudflare.com/",
  networkConfigUrl: "https://mensura.cdn-apple.com/api/v1/gm/config",
  stunServer: "stun.voipgate.com:3478",
  logLevel: "info",
  enableFakeIp: false,
  connectionAskEnabled: false,
  hideOnClose: true,
};

class SettingsStore {
  settings = $state<Settings>({ ...defaultSettings });
  ready = $state(false);
  #loading: Promise<void> | null = null;

  /**
   * Idempotent and awaitable: concurrent callers share one load, so startup can
   * `await` settings before reading a toggle (e.g. autoUpdateGeoIp) without
   * re-running the autostart/hide-on-close sync.
   */
  init(): Promise<void> {
    if (typeof window === "undefined" || this.ready) return Promise.resolve();
    return (this.#loading ??= this.#load());
  }

  async #load() {
    try {
      const stored = localStorage.getItem(SETTINGS_STORAGE_KEY);
      if (stored) {
        const parsed = JSON.parse(stored);
        this.settings = { ...defaultSettings, ...parsed };
      }

      // Sync real OS autostart state with the UI state.
      const autostartEnabled = await isEnabled();
      this.settings.launchAtLogin = autostartEnabled;

      // Sync hide on close to rust backend
      await coreSetHideOnClose(this.settings.hideOnClose);
    } catch (e) {
      console.error("Failed to load settings:", e);
    }

    this.ready = true;
  }

  save() {
    localStorage.setItem(SETTINGS_STORAGE_KEY, JSON.stringify(this.settings));
  }
}

export const settingsStore = new SettingsStore();
