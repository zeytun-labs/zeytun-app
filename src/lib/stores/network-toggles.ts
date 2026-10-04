// One in-flight restart across every entry point (topbar and Control Center).
export function runNetworkToggle<T extends string>(
  current: () => T | null,
  set: (kind: T | null) => void,
  action: () => Promise<void>,
  refresh: () => Promise<void>,
) {
  return async (kind: T) => {
    if (current()) return;
    set(kind);
    try {
      await action();
    } finally {
      try {
        await refresh();
      } finally {
        set(null);
      }
    }
  };
}
