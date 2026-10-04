import { test, expect } from "vitest";
import { runNetworkToggle } from "./network-toggles.ts";

test("holds both controls until core is ready and profile refreshed", async () => {
  let release!: () => void;
  const action = new Promise<void>((resolve) => { release = resolve; });
  let active: string | null = null;
  let refreshed = false;
  const toggle = runNetworkToggle(
    () => active,
    (kind) => { active = kind; },
    async () => { await action; },
    async () => { refreshed = true; },
  );
  const pending = toggle("tun");
  expect(active).toBe("tun");
  await toggle("systemProxy");
  expect(active).toBe("tun");
  expect(refreshed).toBe(false);
  release();
  await pending;
  expect(refreshed).toBe(true);
  expect(active).toBe(null);
});

test("failure unlocks controls and still refreshes persisted state", async () => {
  let active: string | null = null;
  let refreshed = false;
  const toggle = runNetworkToggle(
    () => active,
    (kind) => { active = kind; },
    async () => { throw Error("restart failed"); },
    async () => { refreshed = true; },
  );
  await expect(toggle("systemProxy")).rejects.toThrow(/restart failed/);
  expect(refreshed).toBe(true);
  expect(active).toBe(null);
});
