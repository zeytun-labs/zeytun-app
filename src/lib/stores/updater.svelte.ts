import { Update } from "@tauri-apps/plugin-updater";
import { invoke } from "@tauri-apps/api/core";
import { relaunch } from "@tauri-apps/plugin-process";
import { toast } from "svelte-sonner";
import { coreNetworkPolicyGet, coreProfileGet } from "$lib/core/api";
import { errorMessage } from "$lib/errors";
import { settingsStore } from "$lib/stores/settings.svelte";

/** Background re-check cadence. Also runs once right after core startup. */
const CHECK_INTERVAL_MS = 6 * 60 * 60 * 1000;
/** Last version we toasted about — so the background loop nags only once per release. */
const NOTIFIED_KEY = "zeytun-update-notified";

export type UpdatePhase =
  | "idle"
  | "checking"
  | "available"
  | "downloading"
  /** Package on disk, waiting for the user to restart into it. */
  | "ready";

/**
 * App-lifetime updater state.
 *
 * Lives outside the component tree so a check or a download keeps running — and
 * stays visible — across route changes and the settings dialog opening and
 * closing. Download and install are deliberately separate steps: the app is a
 * VPN client, so restarting it kills the tunnel and must be the user's call.
 */
class UpdaterStore {
  phase = $state<UpdatePhase>("idle");
  update = $state<Update | null>(null);
  /** 0..1 during "downloading". */
  progress = $state(0);
  /** Proxy the update itself should use, resolved when the check runs. */
  proxy = $state<string | null>(null);
  #timer: ReturnType<typeof setInterval> | null = null;

  /** A newer version exists — whether or not it is downloaded yet. */
  get available(): boolean {
    return this.update !== null && this.phase !== "checking";
  }

  /** Downloaded and installable right now. */
  get ready(): boolean {
    return this.phase === "ready" && this.update !== null;
  }

  get version(): string | null {
    return this.update?.version ?? null;
  }

  /**
   * The `app_update` route as a proxy URL, or null for direct.
   *
   * The port is read from the profile rather than hardcoded, and the whole thing
   * degrades to direct if anything is unreadable — a broken settings read must
   * not be able to block updates entirely.
   */
  async #resolveProxy(): Promise<string | null> {
    try {
      const [policies, profile] = await Promise.all([
        coreNetworkPolicyGet(),
        coreProfileGet(),
      ]);
      const policy = policies.app_update;
      if (!policy || policy.kind === "direct") return null;
      const port = profile.local_proxy?.mixed_port ?? 6060;
      return `http://127.0.0.1:${port}`;
    } catch (e) {
      console.error("Failed to resolve update route, using direct:", e);
      return null;
    }
  }

  async #checkChannel(proxy: string | null): Promise<Update | null> {
    const metadata = await invoke<ConstructorParameters<typeof Update>[0] | null>(
      "check_release_update", {
        proxy,
        includePrerelease: settingsStore.settings.receivePrereleaseUpdates,
      },
    );
    return metadata ? new Update(metadata) : null;
  }

  /**
   * @param silent background check — stays quiet unless a *new* version shows up.
   *   Interactive checks (the About button) report up-to-date and failures.
   */
  async check(silent = false) {
    // Never re-check over work in flight, and never throw away a downloaded package.
    if (this.phase !== "idle" && this.phase !== "available") return;
    if (silent && this.available) return; // already surfaced
    this.phase = "checking";
    const previous = this.update;
    this.update = null;
    try {
      if (previous) await previous.close();
      this.proxy = await this.#resolveProxy();
      try {
        this.update = await this.#checkChannel(this.proxy);
      } catch (e) {
        // A dead proxy would otherwise block the very update that fixes it.
        // ponytail: one retry, no backoff — add a real policy chain if a second
        // route ever becomes configurable.
        if (!this.proxy) throw e;
        console.error("Update check via proxy failed, retrying direct:", e);
        this.update = await this.#checkChannel(null);
        this.proxy = null;
        if (!silent) toast.warning("Checked via direct connection — the proxy route failed");
      }
      if (this.update) {
        this.phase = "available";
        if (silent) this.#notifyAvailable(this.update.version);
      } else {
        this.phase = "idle";
        if (!silent) toast.success(settingsStore.settings.receivePrereleaseUpdates
          ? "Zeytun is up to date"
          : "No stable update available");
      }
    } catch (e) {
      this.phase = "idle";
      if (!silent) toast.error("Update check failed", { description: errorMessage(e) });
      else console.error("Background update check failed:", e);
    }
  }

  /** Toast once per version; the sidebar badge is what persists afterwards. */
  #notifyAvailable(version: string) {
    try {
      if (localStorage.getItem(NOTIFIED_KEY) === version) return;
      localStorage.setItem(NOTIFIED_KEY, version);
    } catch {
      // private mode / quota — toast anyway, worst case it repeats
    }
    toast.info(`Zeytun ${version} is available`, {
      duration: 15_000,
      action: { label: "Download", onClick: () => void this.download() },
    });
  }

  /** Fetch the package. Survives navigation; install() is a separate user action. */
  async download() {
    if (!this.update || this.phase !== "available") return;
    this.phase = "downloading";
    this.progress = 0;
    let total = 0;
    let got = 0;
    try {
      await this.update.download((e) => {
        if (e.event === "Started") total = e.data.contentLength ?? 0;
        if (e.event === "Progress") {
          got += e.data.chunkLength;
          this.progress = total ? got / total : 0;
        }
      });
      this.phase = "ready";
      this.progress = 1;
      toast.success(`Zeytun ${this.update.version} is ready to install`, {
        duration: 15_000,
        action: { label: "Restart", onClick: () => void this.install() },
      });
    } catch (e) {
      this.phase = "available";
      this.progress = 0;
      toast.error("Update download failed", { description: errorMessage(e) });
    }
  }

  /** Apply the downloaded package and relaunch. Requires phase "ready". */
  async install() {
    if (!this.update || this.phase !== "ready") return;
    try {
      await this.update.install();
      await relaunch();
    } catch (e) {
      toast.error("Update install failed", { description: errorMessage(e) });
    }
  }

  /** Called once after core startup: check now, then every CHECK_INTERVAL_MS. */
  startAuto() {
    if (this.#timer) return;
    void this.check(true);
    this.#timer = setInterval(() => void this.check(true), CHECK_INTERVAL_MS);
  }

  stopAuto() {
    if (this.#timer) clearInterval(this.#timer);
    this.#timer = null;
  }
}

export const updaterStore = new UpdaterStore();
