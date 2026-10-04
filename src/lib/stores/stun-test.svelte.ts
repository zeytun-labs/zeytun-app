import {
  coreStunTestStart,
  coreStunTestCancel,
  type StunTestProgressEvent,
} from "$lib/core/api";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { settingsStore } from "$lib/stores/settings.svelte";
import { errorMessage } from "$lib/errors";

type TestState = "Idle" | "Testing" | "Completed" | "Error";

const DEFAULT_SERVER = "stun.voipgate.com:3478";
const DEFAULT_OUTBOUND_TAG = "direct";
const WATCHDOG_MS = 30_000;

class StunTestStore {
  testState = $state<TestState>("Idle");
  testProgress = $state<StunTestProgressEvent | null>(null);

  #unlisten: UnlistenFn | null = null;
  #setupPromise: Promise<void> | null = null;
  #watchdog: ReturnType<typeof setTimeout> | null = null;

  async #setupListener() {
    if (this.#unlisten) return;

    this.#unlisten = await listen<StunTestProgressEvent>(
      "stun-test-update",
      (event) => {
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
  }

  #cleanup() {
    if (this.#watchdog !== null) {
      clearTimeout(this.#watchdog);
      this.#watchdog = null;
    }
    this.#unlisten?.();
    this.#unlisten = null;
    this.#setupPromise = null;
  }

  async startTest(opts?: { server?: string; outboundTag?: string }) {
    if (this.testState === "Testing") return;

    const server = opts?.server || settingsStore.settings.stunServer || DEFAULT_SERVER;
    const outboundTag = opts?.outboundTag || DEFAULT_OUTBOUND_TAG;

    this.testState = "Testing";
    this.testProgress = null;

    if (!this.#setupPromise) {
      this.#setupPromise = this.#setupListener();
    }
    await this.#setupPromise;

    try {
      await coreStunTestStart(server, outboundTag);

      this.#watchdog = setTimeout(() => {
        if (this.testState === "Testing") {
          this.testState = "Error";
          this.testProgress = {
            phase: 0,
            externalAddr: "",
            latencyMs: 0,
            natMapping: 0,
            natFiltering: 0,
            isFinal: true,
            error: "STUN test timed out",
            natTypeSupported: false,
          };
        }
        this.#cleanup();
      }, WATCHDOG_MS);
    } catch (e) {
      console.error("STUN test failed to start", e);
      this.testState = "Error";
      this.testProgress = {
        phase: 0,
        externalAddr: "",
        latencyMs: 0,
        natMapping: 0,
        natFiltering: 0,
        isFinal: true,
        error: errorMessage(e),
        natTypeSupported: false,
      };
      this.#cleanup();
    }
  }

  async cancelTest() {
    if (this.testState === "Testing") {
      try {
        await coreStunTestCancel();
        this.testState = "Idle";
        this.testProgress = null;
        this.#cleanup();
      } catch (e) {
        console.error("Failed to cancel STUN test", e);
      }
    }
  }

  reset() {
    this.#cleanup();
    this.testState = "Idle";
    this.testProgress = null;
  }

  destroy() {
    this.#cleanup();
  }
}

export const stunTestStore = new StunTestStore();

if (import.meta.hot) {
  import.meta.hot.dispose(() => {
    stunTestStore.destroy();
  });
}
