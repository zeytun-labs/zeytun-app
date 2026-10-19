import { expect, test } from "vitest";
import { createActionQueue } from "./action-queue";

test("queued mutations read the state left by the preceding mutation", async () => {
  const queue = createActionQueue();
  let finish!: () => void;
  const pending = new Promise<void>((resolve) => { finish = resolve; });
  let state = { first: true, second: true };
  const first = queue.run(async () => { await pending; state = { ...state, first: false }; });
  const second = queue.run(async () => { state = { ...state, second: false }; });
  expect(state).toEqual({ first: true, second: true });
  finish();
  await Promise.all([first, second]);
  expect(state).toEqual({ first: false, second: false });
});

test("a failed mutation is reported without preventing the next action", async () => {
  const queue = createActionQueue();
  const first = queue.run(async () => { throw new Error("Save failed"); });
  const next = queue.run(async () => "saved");
  await expect(first).rejects.toThrow("Save failed");
  await expect(next).resolves.toBe("saved");
});


test("queued work cannot apply an old selection to another profile", async () => {
  let profile = "first";
  const queue = createActionQueue(() => profile);
  const queued = queue.run(async () => "must not run");
  profile = "second";
  await expect(queued).rejects.toThrow("active profile changed");
  await expect(queue.run(async () => "new profile")).resolves.toBe("new profile");
});
