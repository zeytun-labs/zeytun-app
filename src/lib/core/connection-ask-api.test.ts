import { beforeEach, expect, test, vi } from "vitest";

const mocks = vi.hoisted(() => ({ invoke: vi.fn(), emit: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ invoke: mocks.invoke }));
vi.mock("@tauri-apps/api/event", () => ({ emit: mocks.emit }));
import { coreDecideConnectionAsk } from "./api";

const decision = { id: "ask-1", outbound: "direct", reject: false, remember: true, process_path: "/Applications/Test.app/test" };

beforeEach(() => {
  vi.resetAllMocks();
  mocks.invoke.mockResolvedValue(undefined);
  mocks.emit.mockResolvedValue(undefined);
});

test.each([true, false])("notifies other webviews after saving a remembered=%s decision", async (remember) => {
  let resolve!: () => void;
  mocks.invoke.mockReturnValue(new Promise<void>((done) => { resolve = done; }));
  const pending = coreDecideConnectionAsk({ ...decision, remember });
  expect(mocks.emit).not.toHaveBeenCalled();
  resolve();
  await pending;
  expect(mocks.emit).toHaveBeenCalledExactlyOnceWith("profile-updated");
});

test("failed decisions do not announce a profile change", async () => {
  mocks.invoke.mockRejectedValue(new Error("Decision failed"));
  await expect(coreDecideConnectionAsk(decision)).rejects.toThrow("Decision failed");
  expect(mocks.emit).not.toHaveBeenCalled();
});

test.each([{ reject: true }, { process_path: "" }])("does not announce a decision that cannot create a rule: %j", async (overrides) => {
  await coreDecideConnectionAsk({ ...decision, ...overrides });
  expect(mocks.emit).not.toHaveBeenCalled();
});

test("an event failure does not leave an already decided connection queued for retry", async () => {
  mocks.emit.mockRejectedValue(new Error("Event delivery failed"));
  const log = vi.spyOn(console, "error").mockImplementation(() => {});
  try {
    await expect(coreDecideConnectionAsk(decision)).resolves.toBeUndefined();
    expect(log).toHaveBeenCalled();
  } finally {
    log.mockRestore();
  }
});
