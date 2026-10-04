import { toast } from "svelte-sonner";
import { coreUpdateGeoipDb, coreAutoUpdateGeoipDb } from "$lib/core/api";
import { errorMessage } from "$lib/errors";

/**
 * GeoIP database download state.
 *
 * Lives outside the component tree for the same reason the updater does: the
 * settings dialog unmounts each section on tab change, so component-local state
 * lost the spinner mid-download and let a second click start a concurrent
 * download over the first.
 */
class GeoipStore {
  updating = $state(false);

  async update() {
    if (this.updating) return;
    this.updating = true;
    try {
      const result = await coreUpdateGeoipDb();
      toast.success(`GeoIP database updated (${result.month})`, {
        description: result.degraded_to_direct
          ? "The selected route failed; downloaded directly instead."
          : undefined,
      });
    } catch (err) {
      toast.error("Failed to update GeoIP database", {
        description: errorMessage(err),
      });
      console.error("geoip update failed", err);
    } finally {
      this.updating = false;
    }
  }

  /**
   * Background refresh on launch: HEAD-probe the newest published month and pull
   * it only when newer than what's installed. Silent by design — a per-launch CDN
   * hiccup or an already-current database must never surface as a failure toast.
   * Guards on `updating` so it never races a manual download started the same
   * second.
   */
  async autoUpdate() {
    if (this.updating) return;
    this.updating = true;
    try {
      const result = await coreAutoUpdateGeoipDb();
      if (result.kind === "updated") {
        toast.success(`GeoIP database updated (${result.month})`, {
          description: result.degraded_to_direct
            ? "The selected route failed; downloaded directly instead."
            : undefined,
        });
      }
      // current / probe_failed / update_failed: stay quiet.
    } catch (err) {
      console.error("geoip auto-update failed", err);
    } finally {
      this.updating = false;
    }
  }
}

export const geoipStore = new GeoipStore();
