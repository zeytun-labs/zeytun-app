import { beforeEach, expect, test, vi } from "vitest";

const state = vi.hoisted(() => ({
  profileStore: {
    profile: { dns: { fake_ip: true }, local_proxy: { mixed_port: 6060 } },
  },
  settingsStore: { settings: { enableFakeIp: false, connectionAskEnabled: false } },
}));
vi.mock("$lib/stores/profile.svelte", () => ({ profileStore: state.profileStore }));
vi.mock("$lib/stores/settings.svelte", () => ({
  settingsStore: state.settingsStore,
  defaultSettings: {},
}));
vi.mock("@tauri-apps/plugin-autostart", () => ({ enable: vi.fn(), disable: vi.fn() }));
vi.mock("$lib/theme.svelte", () => ({ themePreference: { value: "dark" } }));
vi.mock("$lib/core/api", () => ({
  coreSetConnectionAsk: vi.fn(), coreSetListenPort: vi.fn(), coreSetLogLevel: vi.fn(),
  coreSetHideOnClose: vi.fn(), coreNetworkPolicyGet: vi.fn(async () => ({})),
  coreNetworkPolicySet: vi.fn(),
}));
import { SettingsDraft } from "./settings-draft.svelte";

beforeEach(() => {
  state.profileStore.profile.dns.fake_ip = true;
  state.settingsStore.settings.enableFakeIp = false;
});

test("a new Settings draft takes FakeIP from the active profile instead of legacy local settings", () => {
  expect(new SettingsDraft().settings.enableFakeIp).toBe(true);
});

test("opening Settings again follows a DNS-page change without rewriting a second source", () => {
  const draft = new SettingsDraft();
  state.profileStore.profile.dns.fake_ip = false;
  state.settingsStore.settings.enableFakeIp = true;
  draft.sync();
  expect(draft.settings.enableFakeIp).toBe(false);
  expect(state.settingsStore.settings.enableFakeIp).toBe(true);
});

test("FakeIP draft dirtiness follows the profile baseline and resets cleanly despite stale local settings", () => {
  const draft = new SettingsDraft();
  draft.sync();
  expect(draft.settings.enableFakeIp).toBe(true);
  expect(state.settingsStore.settings.enableFakeIp).toBe(false);
  expect(draft.hasChanges).toBe(false);

  draft.settings.enableFakeIp = false;
  expect(draft.hasChanges).toBe(true);

  draft.sync();
  expect(draft.settings.enableFakeIp).toBe(true);
  expect(draft.hasChanges).toBe(false);
});
