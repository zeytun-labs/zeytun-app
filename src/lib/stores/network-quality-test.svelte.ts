import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import {
  coreNetworkQualityTestStart,
  coreNetworkQualityTestCancel,
  type NetworkQualityTestProgressEvent,
} from "$lib/core/api";
import { settingsStore } from "$lib/stores/settings.svelte";
import { errorMessage } from "$lib/errors";

type TestState = "Idle" | "Testing" | "Completed" | "Error";

const EMPTY_PROGRESS: NetworkQualityTestProgressEvent = {
  phase: 0,
  downloadCapacity: 0,
  uploadCapacity: 0,
  downloadRpm: 0,
  uploadRpm: 0,
  idleLatencyMs: 0,
  elapsedMs: 0,
  isFinal: false,
  error: "",
  downloadCapacityAccuracy: 0,
  uploadCapacityAccuracy: 0,
  downloadRpmAccuracy: 0,
  uploadRpmAccuracy: 0,
};

const APPLE_MENSURA_CONFIG_URL =
  "https://mensura.cdn-apple.com/api/v1/gm/config";
const DEFAULT_OUTBOUND_TAG = "direct";
const DEFAULT_MAX_RUNTIME_SECONDS = 20;
// Safety margin past the backend runtime: if the daemon dies without emitting a
// final/error event, force cleanup so the listener never leaks.
const WATCHDOG_GRACE_SECONDS = 15;

class NetworkQualityTestStore {
  testState = $state<TestState>("Idle");
  testProgress = $state<NetworkQualityTestProgressEvent | null>(null);

  #generation = 0;
  #unlisten: UnlistenFn | null = null;
  #setupPromise: Promise<void> | null = null;
  #watchdog: ReturnType<typeof setTimeout> | null = null;

  async #setupListener(generation: number) {
    if (this.#unlisten) return; // Already setup

    const unlisten = await listen<NetworkQualityTestProgressEvent>(
      "network-test-update",
      (event) => {
        if (generation !== this.#generation) return;
        this.testProgress = event.payload;

        if (event.payload.isFinal) {
          this.testState = event.payload.error ? "Error" : "Completed";
          this.#cleanup();
        } else if (event.payload.error) {
          this.testState = "Error";
          this.#cleanup();
        }
      },
    );
    if (generation !== this.#generation) unlisten();
    else this.#unlisten = unlisten;
  }

  #cleanup() {
    ++this.#generation;
    if (this.#watchdog !== null) {
      clearTimeout(this.#watchdog);
      this.#watchdog = null;
    }
    this.#unlisten?.();
    this.#unlisten = null;
    this.#setupPromise = null;
  }

  async startTest(opts?: {
    configUrl?: string;
    outboundTag?: string;
    serial?: boolean;
    maxRuntimeSeconds?: number;
    http3?: boolean;
  }) {
    // Re-entry guard: a test is already running. Without this a second call
    // would leave the previous listener/watchdog dangling.
    if (this.testState === "Testing") return;

    const configUrl = opts?.configUrl || settingsStore.settings.networkConfigUrl || APPLE_MENSURA_CONFIG_URL;
    const outboundTag = opts?.outboundTag || DEFAULT_OUTBOUND_TAG;
    const serial = opts?.serial ?? false;
    const maxRuntimeSeconds =
      opts?.maxRuntimeSeconds ?? DEFAULT_MAX_RUNTIME_SECONDS;
    const http3 = opts?.http3 ?? false;

    const generation = ++this.#generation;
    this.testState = "Testing";
    this.testProgress = null;

    try {
      this.#setupPromise ??= this.#setupListener(generation);
      await this.#setupPromise;
      if (generation !== this.#generation) return;
      await coreNetworkQualityTestStart(
        configUrl,
        outboundTag,
        serial,
        maxRuntimeSeconds,
        http3,
      );

      if (generation !== this.#generation || this.testState !== "Testing") return;

      // Arm the watchdog only once the backend confirmed it started.
      this.#watchdog = setTimeout(
        () => {
          if (this.testState === "Testing") {
            this.testState = "Error";
            this.testProgress = {
              ...(this.testProgress ?? EMPTY_PROGRESS),
              error: "Network quality test timed out",
              isFinal: true,
            };
          }
          this.#cleanup();
        },
        (maxRuntimeSeconds + WATCHDOG_GRACE_SECONDS) * 1000,
      );
    } catch (e) {
      if (generation !== this.#generation) return;
      console.error("Test failed to start", e);
      this.testState = "Error";
      this.testProgress = {
        ...EMPTY_PROGRESS,
        error: errorMessage(e),
        isFinal: true,
      };
      this.#cleanup();
    }
  }

  async cancelTest() {
    if (this.testState === "Testing") {
      try {
        await coreNetworkQualityTestCancel();
        this.testState = "Idle";
        this.testProgress = null;
        this.#cleanup();
      } catch (e) {
        console.error("Failed to cancel test", e);
      }
    }
  }

  reset() {
    this.#cleanup();
    this.testState = "Idle";
    this.testProgress = null;
  }

  /** Call on component unmount or HMR */
  destroy() {
    this.#cleanup();
  }
}

export const networkQualityTestStore = new NetworkQualityTestStore();

// HMR cleanup
if (import.meta.hot) {
  import.meta.hot.dispose(() => {
    networkQualityTestStore.destroy();
  });
}
