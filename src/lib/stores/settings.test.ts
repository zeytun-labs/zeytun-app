import { describe, expect, it, vi } from "vitest";

vi.mock("@tauri-apps/plugin-autostart", () => ({
  enable: vi.fn(),
  disable: vi.fn(),
  isEnabled: vi.fn(async () => false),
}));
vi.mock("$lib/core/api", () => ({ coreSetHideOnClose: vi.fn() }));

import { SETTINGS_VERSION, defaultSettings, migrateSettings } from "./settings.svelte";

describe("migrateSettings", () => {
  it("turns pre-release updates on for settings saved before versioning", () => {
    const migrated = migrateSettings({ receivePrereleaseUpdates: false, logLevel: "debug" });
    expect(migrated.receivePrereleaseUpdates).toBe(true);
    expect(migrated.logLevel).toBe("debug");
    expect(migrated.settingsVersion).toBe(SETTINGS_VERSION);
  });

  it("keeps an explicit opt-out made after the migration", () => {
    const migrated = migrateSettings({
      receivePrereleaseUpdates: false,
      settingsVersion: SETTINGS_VERSION,
    });
    expect(migrated.receivePrereleaseUpdates).toBe(false);
  });

  it("defaults fresh installs to pre-release updates", () => {
    expect(defaultSettings.receivePrereleaseUpdates).toBe(true);
    expect(migrateSettings({}).receivePrereleaseUpdates).toBe(true);
  });
});
