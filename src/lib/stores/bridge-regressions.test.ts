// Regression coverage for native store synchronization and protobuf transport.
// Native IPC and WebSocket events are mocked; store and protobuf code are real.
import { afterEach, beforeEach, expect, test, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  listen: vi.fn(),
  api: Object.fromEntries([
    "coreProfileGet", "coreProfileList", "coreProfileCreate", "coreProfileUpdate",
    "coreProfileDelete", "coreProfileSwitch", "coreProfileRefresh", "coreProfileMarkSummaryRead", "coreStatus",
    "coreSubscriptionList", "coreSubscriptionCreate", "coreSubscriptionUpdate", "coreSubscriptionRefresh",
    "coreSubscriptionRefreshAll", "coreSubscriptionDeletePreview", "coreSubscriptionDelete", "coreSubscriptionMarkSummaryRead",
    "coreNetworkQualityTestStart", "coreNetworkQualityTestCancel", "coreStunTestStart", "coreStunTestCancel",
  ].map((name) => [name, vi.fn()])),
}));
vi.mock("@tauri-apps/api/event", () => ({ listen: mocks.listen }));
vi.mock("$lib/core/api", () => mocks.api);
vi.mock("./settings.svelte", () => ({ settingsStore: { settings: { stunServer: "example.test:3478", networkConfigUrl: "https://example.test/config" } } }));

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason: Error) => void;
  const promise = new Promise<T>((done, fail) => { resolve = done; reject = fail; });
  return { promise, resolve, reject };
}
function snapshot(id: string) {
  return { profile: { id, policies: [], proxies: [] }, runtime: { phase: "idle", pending_apply: false }, profiles: [{ id, is_active: true }] };
}
afterEach(() => { vi.unstubAllGlobals(); });

beforeEach(() => {
  vi.resetModules();
  vi.resetAllMocks();
  mocks.listen.mockResolvedValue(vi.fn());
  const current = snapshot("B");
  mocks.api.coreProfileGet.mockResolvedValue(current.profile);
  mocks.api.coreStatus.mockResolvedValue(current.runtime);
  mocks.api.coreProfileList.mockResolvedValue(current.profiles);
});

test("a late old profile refresh must not overwrite the newer active snapshot", async () => {
  const { profileStore } = await import("./profile.svelte");
  const old = snapshot("A");
  const oldProfile = deferred<typeof old.profile>();
  const oldRuntime = deferred<typeof old.runtime>();
  const oldRegistry = deferred<typeof old.profiles>();
  mocks.api.coreProfileGet.mockReturnValueOnce(oldProfile.promise);
  mocks.api.coreStatus.mockReturnValueOnce(oldRuntime.promise);
  mocks.api.coreProfileList.mockReturnValueOnce(oldRegistry.promise);
  const pending = profileStore.refresh();
  await profileStore.refresh();
  expect(profileStore.profile?.id).toBe("B");
  oldProfile.resolve(old.profile); oldRuntime.resolve(old.runtime); oldRegistry.resolve(old.profiles);
  await pending;
  expect(profileStore.profile?.id).toBe("B");
});

test("concurrent profile listener initialization must not duplicate listeners", async () => {
  const { profileStore } = await import("./profile.svelte");
  const first = deferred<() => void>();
  const second = deferred<() => void>();
  mocks.listen.mockReturnValueOnce(first.promise).mockReturnValueOnce(second.promise);
  const a = profileStore.startListening();
  const b = profileStore.startListening();
  first.resolve(vi.fn()); second.resolve(vi.fn());
  await Promise.all([a,b]);
  profileStore.stopListening();
  expect(mocks.listen.mock.calls.filter(([name]) => name === "profile-updated")).toHaveLength(1);
  expect(mocks.listen.mock.calls.filter(([name]) => name === "core-status-changed")).toHaveLength(1);
});

test("profile listener teardown must cancel registration still in flight", async () => {
  const { profileStore } = await import("./profile.svelte");
  const late = deferred<() => void>();
  const unlisten = vi.fn();
  mocks.listen.mockReturnValueOnce(late.promise);
  const pending = profileStore.startListening();
  profileStore.stopListening();
  late.resolve(unlisten);
  await pending;
  expect(unlisten).toHaveBeenCalled();
});

test("a late same-profile subscription list must not overwrite newer sources", async () => {
  const { subscriptionsStore } = await import("./subscriptions.svelte");
  const oldRead = deferred<any>();
  mocks.api.coreSubscriptionList.mockReturnValueOnce(oldRead.promise).mockResolvedValue([{id:"new-source"}]);
  const pending = subscriptionsStore.load("A");
  await subscriptionsStore.load("A");
  expect(subscriptionsStore.sources[0].id).toBe("new-source");
  oldRead.resolve([{id:"deleted-source"}]);
  await pending;
  expect(subscriptionsStore.sources[0].id).toBe("new-source");
});

test.each([
  ["network quality", "./network-quality-test.svelte", "networkQualityTestStore"],
  ["STUN", "./stun-test.svelte", "stunTestStore"],
])("%s listener rejection must clear the Testing state", async (_, path, name) => {
  mocks.listen.mockRejectedValueOnce(new Error("event IPC rejected"));
  const module = await import(path);
  const store = module[name];
  await expect(store.startTest()).resolves.toBeUndefined();
  expect(store.testProgress.error).toBe("event IPC rejected");
  expect(store.testState).toBe("Error");
});

test("a final network quality error must remain an Error state", async () => {
  let handler: any;
  mocks.listen.mockImplementation(async (_name, callback) => { handler = callback; return vi.fn(); });
  mocks.api.coreNetworkQualityTestStart.mockResolvedValue(undefined);
  const { networkQualityTestStore } = await import("./network-quality-test.svelte");
  await networkQualityTestStore.startTest();
  handler({payload:{isFinal:true,error:"core stream failed"}});
  expect(networkQualityTestStore.testState).toBe("Error");
});


test("load and refresh share a generation, including loading state and bootstrap status", async () => {
  const { profileStore } = await import("./profile.svelte");
  const old = snapshot("A");
  const slow = deferred<typeof old.profile>();
  mocks.api.coreProfileGet.mockReturnValueOnce(slow.promise);
  mocks.api.coreStatus.mockResolvedValueOnce({ phase: "running" });
  const loading = profileStore.load();
  await profileStore.refresh();
  expect(profileStore.loading).toBe(false);
  slow.resolve(old.profile);
  await loading;
  expect(profileStore.profile?.id).toBe("B");
  expect(profileStore.coreStatus).toBe("connecting");
});

test("a stale load finishing cannot clear a newer load's spinner", async () => {
  const { profileStore } = await import("./profile.svelte");
  const first = deferred<any>();
  const second = deferred<any>();
  mocks.api.coreProfileGet.mockReturnValueOnce(first.promise).mockReturnValueOnce(second.promise);
  const a = profileStore.load();
  const b = profileStore.load();
  first.resolve(snapshot("A").profile);
  await a;
  expect(profileStore.loading).toBe(true);
  second.resolve(snapshot("B").profile);
  await b;
  expect(profileStore.loading).toBe(false);
});

test("a live status event takes precedence over bootstrap read-back", async () => {
  const { profileStore } = await import("./profile.svelte");
  let statusHandler: any;
  mocks.listen.mockImplementation(async (name, handler) => {
    if (name === "core-status-changed") statusHandler = handler;
    return vi.fn();
  });
  await profileStore.startListening();
  const pending = deferred<any>();
  mocks.api.coreStatus.mockReturnValueOnce(pending.promise);
  const loading = profileStore.load();
  statusHandler({ payload: "stopped" });
  pending.resolve({ phase: "running" });
  await loading;
  expect(profileStore.coreStatus).toBe("stopped");
  profileStore.stopListening();
});

test("failed partial registration rolls back and allows a later retry", async () => {
  const { profileStore } = await import("./profile.svelte");
  const unlisten = vi.fn();
  const error = vi.spyOn(console, "error").mockImplementation(() => {});
  mocks.listen.mockResolvedValueOnce(unlisten).mockRejectedValueOnce(new Error("status IPC failed"));
  await expect(profileStore.startListening()).resolves.toBeUndefined();
  expect(unlisten).toHaveBeenCalledTimes(1);
  await profileStore.startListening();
  expect(mocks.listen).toHaveBeenCalledTimes(4);
  profileStore.stopListening();
  error.mockRestore();
});

test("teardown while the second registration is pending cleans both handles", async () => {
  const { profileStore } = await import("./profile.svelte");
  const late = deferred<() => void>();
  const first = vi.fn();
  const second = vi.fn();
  mocks.listen.mockResolvedValueOnce(first).mockReturnValueOnce(late.promise);
  const pending = profileStore.startListening();
  await vi.waitFor(() => expect(mocks.listen).toHaveBeenCalledTimes(2));
  profileStore.stopListening();
  expect(first).toHaveBeenCalledTimes(1);
  late.resolve(second);
  await pending;
  expect(first).toHaveBeenCalledTimes(1);
  expect(second).toHaveBeenCalledTimes(1);
});

test("a cancelled registration cannot replace handles from a new mount", async () => {
  const { profileStore } = await import("./profile.svelte");
  const late = deferred<() => void>();
  const oldHandle = vi.fn();
  const newProfile = vi.fn();
  const newStatus = vi.fn();
  mocks.listen.mockReturnValueOnce(late.promise).mockResolvedValueOnce(newProfile).mockResolvedValueOnce(newStatus);
  const old = profileStore.startListening();
  profileStore.stopListening();
  await profileStore.startListening();
  late.resolve(oldHandle);
  await old;
  expect(oldHandle).toHaveBeenCalledTimes(1);
  expect(newProfile).not.toHaveBeenCalled();
  profileStore.stopListening();
  expect(newProfile).toHaveBeenCalledTimes(1);
  expect(newStatus).toHaveBeenCalledTimes(1);
});

test("profile-updated callbacks handle refresh rejections", async () => {
  const { profileStore } = await import("./profile.svelte");
  let onUpdate: any;
  mocks.listen.mockImplementation(async (name, handler) => {
    if (name === "profile-updated") onUpdate = handler;
    return vi.fn();
  });
  const error = vi.spyOn(console, "error").mockImplementation(() => {});
  await profileStore.startListening();
  mocks.api.coreProfileGet.mockRejectedValueOnce(new Error("profile IPC failed"));
  onUpdate();
  await vi.waitFor(() => expect(error).toHaveBeenCalledWith("Profile refresh failed", expect.any(Error)));
  profileStore.stopListening();
  error.mockRestore();
});

test("stale subscription errors and finalizers do not affect the newer request", async () => {
  const { subscriptionsStore } = await import("./subscriptions.svelte");
  const old = deferred<any>();
  const current = deferred<any>();
  mocks.api.coreSubscriptionList.mockReturnValueOnce(old.promise).mockReturnValueOnce(current.promise);
  const a = subscriptionsStore.load("A");
  const b = subscriptionsStore.load("B");
  old.reject(new Error("old profile failed"));
  await a;
  expect(subscriptionsStore.loading).toBe(true);
  expect(subscriptionsStore.error).toBeNull();
  current.resolve([{ id: "new-source" }]);
  await b;
  expect(subscriptionsStore.sources[0].id).toBe("new-source");
  expect(subscriptionsStore.loading).toBe(false);
});

test("clearing the active profile invalidates pending subscription reads", async () => {
  const { subscriptionsStore } = await import("./subscriptions.svelte");
  const slow = deferred<any>();
  mocks.api.coreSubscriptionList.mockReturnValueOnce(slow.promise);
  const pending = subscriptionsStore.load("A");
  await subscriptionsStore.load();
  slow.resolve([{ id: "stale-source" }]);
  await pending;
  expect(subscriptionsStore.sources).toEqual([]);
  expect(subscriptionsStore.activeProfileId).toBeNull();
  expect(subscriptionsStore.loading).toBe(false);
});

test.each([
  ["network quality", "./network-quality-test.svelte", "networkQualityTestStore", "coreNetworkQualityTestStart"],
  ["STUN", "./stun-test.svelte", "stunTestStore", "coreStunTestStart"],
])("%s listener setup can be retried after failure", async (_, path, name, command) => {
  const error = vi.spyOn(console, "error").mockImplementation(() => {});
  mocks.listen.mockRejectedValueOnce(new Error("event IPC rejected"));
  const module = await import(path);
  const store = module[name];
  await store.startTest();
  expect(store.testState).toBe("Error");
  await store.startTest();
  expect(store.testState).toBe("Testing");
  expect(mocks.api[command]).toHaveBeenCalledTimes(1);
  store.reset();
  error.mockRestore();
});

test.each([
  ["network quality", "./network-quality-test.svelte", "networkQualityTestStore", "coreNetworkQualityTestStart"],
  ["STUN", "./stun-test.svelte", "stunTestStore", "coreStunTestStart"],
])("%s reset cancels a pending listener without starting the backend", async (_, path, name, command) => {
  const late = deferred<() => void>();
  const unlisten = vi.fn();
  mocks.listen.mockReturnValueOnce(late.promise);
  const module = await import(path);
  const store = module[name];
  const pending = store.startTest();
  store.reset();
  late.resolve(unlisten);
  await pending;
  expect(unlisten).toHaveBeenCalledTimes(1);
  expect(store.testState).toBe("Idle");
  expect(mocks.api[command]).not.toHaveBeenCalled();
});
