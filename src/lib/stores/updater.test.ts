import { afterEach, beforeEach, expect, test, vi } from "vitest";
import type { DownloadEvent } from "$lib/core/release-update";

const mocks = vi.hoisted(() => ({
  invoke: vi.fn(), close: vi.fn(), relaunch: vi.fn(), channels: [] as { onmessage: (e: DownloadEvent) => void }[],
  networkPolicy: vi.fn(), profile: vi.fn(), notification: vi.fn(),
  toast: { success: vi.fn(), error: vi.fn(), info: vi.fn(), warning: vi.fn() },
}));
vi.mock("@tauri-apps/api/core", () => ({
  invoke: mocks.invoke,
  Resource: class {
    constructor(readonly rid: number) {}
    async close() { await mocks.close(this.rid); }
  },
  Channel: class {
    onmessage!: (event: DownloadEvent) => void;
    constructor() { mocks.channels.push(this); }
  },
}));
vi.mock("@tauri-apps/plugin-process", () => ({ relaunch: mocks.relaunch }));
vi.mock("svelte-sonner", () => ({ toast: mocks.toast }));
vi.mock("$lib/core/api", () => ({ coreNetworkPolicyGet: mocks.networkPolicy, coreProfileGet: mocks.profile }));
vi.mock("./settings.svelte", () => ({ settingsStore: { settings: { receivePrereleaseUpdates: false } } }));
vi.mock("./notifications.svelte", () => ({ notificationStore: { pushNotification: mocks.notification }, SCOPE_APP: 0 }));

const metadata = { rid: 42, version: "3.0.0", currentVersion: "2.0.0" };
function deferred() {
  let resolve!: () => void;
  let reject!: (error: Error) => void;
  const promise = new Promise<void>((done, fail) => { resolve = done; reject = fail; });
  return { promise, resolve, reject };
}
beforeEach(() => {
  vi.resetModules(); vi.resetAllMocks(); mocks.channels.length = 0;
  mocks.networkPolicy.mockResolvedValue({ app_update: { kind: "direct" } });
  mocks.profile.mockResolvedValue({ local_proxy: { mixed_port: 6060 } });
  mocks.invoke.mockImplementation(async (command) => command === "check_release_update" ? metadata : undefined);
});
afterEach(() => vi.useRealTimers());

test("About uses checked native resources for the entire update flow", async () => {
  const { updaterStore } = await import("./updater.svelte");
  await updaterStore.check();
  expect(mocks.invoke).toHaveBeenCalledWith("check_release_update", { proxy: null, includePrerelease: false });
  expect(updaterStore.phase).toBe("available");
  await updaterStore.download();
  expect(mocks.invoke).toHaveBeenCalledWith("download_release_update", { rid: 42, onEvent: mocks.channels[0] });
  expect(updaterStore.phase).toBe("ready");
  await updaterStore.install();
  expect(mocks.invoke).toHaveBeenCalledWith("install_release_update", { rid: 42 });
  expect(mocks.relaunch).toHaveBeenCalledTimes(1);
  expect(mocks.invoke.mock.calls.every(([command]) => !command.startsWith("plugin:updater|"))).toBe(true);
});

test("progress reaching 100 percent cannot mark an unverified package ready", async () => {
  const { updaterStore } = await import("./updater.svelte");
  await updaterStore.check();
  const pending = deferred(); mocks.invoke.mockReturnValueOnce(pending.promise);
  const downloading = updaterStore.download();
  mocks.channels[0].onmessage({ event: "Started", data: { contentLength: 10 } });
  mocks.channels[0].onmessage({ event: "Progress", data: { chunkLength: 10 } });
  expect(updaterStore.progress).toBe(1);
  expect(updaterStore.phase).toBe("downloading");
  await updaterStore.install();
  expect(mocks.invoke).not.toHaveBeenCalledWith("install_release_update", expect.anything());
  pending.reject(new Error("Signed bundle version does not match release version"));
  await downloading;
  expect(updaterStore.phase).toBe("available");
  expect(updaterStore.ready).toBe(false);
  expect(mocks.relaunch).not.toHaveBeenCalled();
});

test("a verified download can be retried after a signature failure", async () => {
  const { updaterStore } = await import("./updater.svelte");
  await updaterStore.check();
  mocks.invoke.mockRejectedValueOnce(new Error("Invalid signature"));
  await updaterStore.download();
  expect(updaterStore.phase).toBe("available");
  await updaterStore.download();
  expect(updaterStore.ready).toBe(true);
});

test("native install rejection prevents relaunch", async () => {
  const { updaterStore } = await import("./updater.svelte");
  await updaterStore.check(); await updaterStore.download();
  mocks.invoke.mockRejectedValueOnce(new Error("Installed version is newer"));
  await updaterStore.install();
  expect(mocks.relaunch).not.toHaveBeenCalled();
  expect(mocks.toast.error).toHaveBeenCalledWith("Update install failed", expect.anything());
});

test("proxy fallback retains the same native verification path", async () => {
  const error = vi.spyOn(console, "error").mockImplementation(() => {});
  mocks.networkPolicy.mockResolvedValue({ app_update: { kind: "local_proxy" } });
  mocks.invoke.mockRejectedValueOnce(new Error("Proxy unavailable")).mockResolvedValueOnce(metadata);
  const { updaterStore } = await import("./updater.svelte");
  await updaterStore.check();
  expect(mocks.invoke.mock.calls.map(([, options]) => options.proxy)).toEqual(["http://127.0.0.1:6060", null]);
  expect(updaterStore.phase).toBe("available");
  error.mockRestore();
});

test("rechecking closes the previous resource and creates no overlapping checks", async () => {
  const { updaterStore } = await import("./updater.svelte");
  await updaterStore.check();
  const pending = deferred(); mocks.close.mockReturnValueOnce(pending.promise);
  const checking = updaterStore.check();
  await updaterStore.check();
  expect(mocks.close).toHaveBeenCalledTimes(1);
  pending.resolve(); await checking;
  expect(mocks.invoke.mock.calls.filter(([name]) => name === "check_release_update")).toHaveLength(2);
});
