import { onDestroy } from "svelte";

export function usePolling(
  callback: () => Promise<void>,
  intervalMs: number,
  onError: (err: unknown) => void = (err) => console.error("[usePolling] callback failed:", err),
) {
  let active = $state(false);
  let intervalId: ReturnType<typeof setInterval> | null = null;

  function start() {
    if (intervalId) return;
    active = true;
    intervalId = setInterval(async () => {
      try {
        await callback();
      } catch (err) {
        onError(err);
      }
    }, intervalMs);
  }

  function stop() {
    if (intervalId) {
      clearInterval(intervalId);
      intervalId = null;
    }
    active = false;
  }

  onDestroy(() => stop());

  return {
    get active() {
      return active;
    },
    start,
    stop,
  };
}
