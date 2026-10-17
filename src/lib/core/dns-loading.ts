/** Keep visual loading separate from actual request completion and its data. */
export function createDnsLoadingIndicator(
  onChange: (visible: boolean) => void,
  delayMs = 140,
  minimumVisibleMs = 200,
) {
  let revealTimer: ReturnType<typeof setTimeout> | undefined;
  let hideTimer: ReturnType<typeof setTimeout> | undefined;
  let visibleSince: number | null = null;
  let disposed = false;

  return {
    start() {
      if (disposed) return;
      clearTimeout(revealTimer);
      clearTimeout(hideTimer);
      if (visibleSince !== null) return;
      revealTimer = setTimeout(() => {
        visibleSince = Date.now();
        onChange(true);
      }, delayMs);
    },
    finish() {
      if (disposed) return;
      clearTimeout(revealTimer);
      clearTimeout(hideTimer);
      if (visibleSince === null) return;
      const remaining = Math.max(0, minimumVisibleMs - (Date.now() - visibleSince));
      hideTimer = setTimeout(() => {
        visibleSince = null;
        onChange(false);
      }, remaining);
    },
    dispose() {
      disposed = true;
      clearTimeout(revealTimer);
      clearTimeout(hideTimer);
    },
  };
}

/** Explicit resolver queries must never trigger a hidden System comparison. */
export function shouldCompareSystem(server: string | null, recordType: string, selected: boolean): boolean {
  return server === null && recordType !== "CNAME" && selected;
}
