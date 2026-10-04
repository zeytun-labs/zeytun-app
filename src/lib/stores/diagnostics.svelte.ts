import { coreRunDiagnostics } from "$lib/core/api";
import type { DiagnosticsResult } from "$lib/core/types";
import { settingsStore } from "$lib/stores/settings.svelte";
import { errorMessage } from "$lib/errors";

/// Drives the Activity dashboard's Network Diagnostics card and its report
/// sheet. A single `run()` triggers the backend suite (router ping, DNS timing,
/// direct + proxy HTTP probes) and stores both the raw numbers and the
/// formatted report.
class DiagnosticsStore {
  result = $state<DiagnosticsResult | null>(null);
  running = $state(false);
  error = $state<string | null>(null);

  /// Whether a diagnostics run has ever completed (used to gate N/A vs "—").
  hasResult = $derived(this.result !== null);

  async run() {
    if (this.running) return;
    this.running = true;
    this.error = null;
    try {
      this.result = await coreRunDiagnostics(
        settingsStore.settings.internetTestUrl,
        settingsStore.settings.proxyTestUrl
      );
    } catch (e) {
      console.error("core_run_diagnostics failed:", e);
      this.error = errorMessage(e);
    } finally {
      this.running = false;
    }
  }

  reset() {
    this.result = null;
    this.error = null;
    this.running = false;
  }
}

export const diagnosticsStore = new DiagnosticsStore();
