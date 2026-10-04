import {
  coreClashProxies,
  coreClashProxyDelayTest,
} from "$lib/core/api";
import { tick } from "svelte";
import type {
  ClashDelayResult,
  CoreProfile,
  Proxy,
} from "$lib/core/types";
import {
  DELAY_TEST_URL,
  DELAY_TEST_TIMEOUT_MS,
  RESERVED_TAGS,
} from "$lib/core/constants";
import { profileStore } from "./profile.svelte";
import { errorMessage } from "$lib/errors";

async function allowUiToPaint() {
  await tick();
  if (typeof requestAnimationFrame === "undefined") return;
  await new Promise<void>((resolve) => requestAnimationFrame(() => resolve()));
}

class LatencyStore {
  latencyMap = $state<Record<string, number | null>>({});
  testingAll = $state(false);
  testingProxyIds = $state<Set<string>>(new Set());

  getLatencyMs(proxy: Proxy): number | null | undefined {
    return this.latencyMap[proxy.tag];
  }

  isTestingProxy(proxyTag: string): boolean {
    return this.testingProxyIds.has(proxyTag) || this.testingAll;
  }

  async runUrlTestAll(proxies: Proxy[]): Promise<string | null> {
    const profile = profileStore.profile;
    if (!profile || !profileStore.isRunning) {
      return !profileStore.isRunning
        ? "Core must be running to perform URL test."
        : "No profile available.";
    }

    const targets = proxies.filter((proxy) => proxy.enabled);
    if (targets.length === 0) {
      return "No enabled proxies in this group.";
    }

    this.testingAll = true;
    this.testingProxyIds = new Set([
      ...this.testingProxyIds,
      ...targets.map((proxy) => proxy.tag),
    ]);
    let errorMsg: string | null = null;

    try {
      await allowUiToPaint();

      const clashData = await coreClashProxies();
      const missing: string[] = [];
      const proxyClashTag = new Map<string, string>();

      for (const proxy of targets) {
        const clashTag = this.#findProxyTagInClash(proxy, clashData.proxies, profile);
        if (clashTag) {
          proxyClashTag.set(proxy.tag, clashTag);
        } else {
          missing.push(proxy.title);
        }
      }

      await Promise.allSettled(
        targets.map(async (proxy) => {
          const clashTag = proxyClashTag.get(proxy.tag);
          if (!clashTag) {
            this.#applyResult(proxy, { delay: null });
            this.#removeFromTesting(proxy.tag);
            return;
          }
          try {
            const result = await coreClashProxyDelayTest(
              clashTag,
              DELAY_TEST_URL,
              DELAY_TEST_TIMEOUT_MS,
            );
            this.#applyResult(proxy, result);
          } catch {
            this.#applyResult(proxy, { delay: null });
          } finally {
            this.#removeFromTesting(proxy.tag);
          }
        }),
      );

      if (missing.length > 0) {
        errorMsg = `Some proxies were not found in clash API: ${missing.join(", ")}`;
      }
    } catch (err) {
      errorMsg = errorMessage(err);
    } finally {
      this.testingAll = false;
    }

    return errorMsg;
  }

  async runUrlTestSingle(proxy: Proxy): Promise<string | null> {
    const profile = profileStore.profile;
    if (!profile || !profileStore.isRunning) {
      return "Core must be running to perform URL test.";
    }

    this.#addToTesting(proxy.tag);
    let errorMsg: string | null = null;

    try {
      await allowUiToPaint();

      const clashData = await coreClashProxies();
      const proxyTag = this.#findProxyTagInClash(proxy, clashData.proxies, profile);
      if (!proxyTag) {
        return `Could not find proxy "${proxy.title}" in clash API.`;
      }
      const result = await coreClashProxyDelayTest(
        proxyTag,
        DELAY_TEST_URL,
        DELAY_TEST_TIMEOUT_MS,
      );
      this.#applyResult(proxy, result);
      // A proxy that did not answer is a result, not an error: the grid already
      // shows `timeout` for it, so no toast.
    } catch (err) {
      this.#applyResult(proxy, { delay: null });
      errorMsg = errorMessage(err);
    } finally {
      this.#removeFromTesting(proxy.tag);
    }

    return errorMsg;
  }

  async fetchLatenciesOnLoad() {
    if (!profileStore.isRunning) return;
    const profile = profileStore.profile;
    if (!profile) return;

    try {
      const clashData = await coreClashProxies();
      const updated: Record<string, number | null> = {};
      const { proxyTags } = this.#allocateRuntimeTags(profile);

      for (const proxy of profile.proxies) {
        const tag = proxyTags[proxy.tag];
        const info = clashData.proxies[tag] as
          | { history?: { delay: number }[] }
          | undefined;
        if (info?.history && info.history.length > 0) {
          const last = info.history[info.history.length - 1];
          updated[proxy.tag] = last.delay > 0 ? last.delay : null;
        }
      }
      this.latencyMap = updated;
    } catch {
    }
  }


  #findProxyTagInClash(
    proxy: Proxy,
    clashProxies: Record<string, unknown>,
    profile: CoreProfile,
  ): string | null {
    if (clashProxies[proxy.tag]) return proxy.tag;
    const expectedTag = this.#allocateRuntimeTags(profile).proxyTags[proxy.tag];
    if (expectedTag && clashProxies[expectedTag]) return expectedTag;
    // Absent from the clash map either way — reporting the tag would call the
    // delay test on a nonexistent proxy and render as a silent "timeout".
    return null;
  }

  #allocateRuntimeTags(currentProfile: CoreProfile) {
    const used = new Set<string>(RESERVED_TAGS);
    const policyTags: Record<string, string> = {};
    const proxyTags: Record<string, string> = {};

    for (const policy of currentProfile.policies) {
      policyTags[policy.tag] = policy.tag;
    }

    for (const proxy of currentProfile.proxies.filter((item) => item.enabled)) {
      proxyTags[proxy.tag] = proxy.tag;
    }

    return { policyTags, proxyTags };
  }

  #applyResult(proxy: Proxy, result: ClashDelayResult) {
    this.latencyMap = {
      ...this.latencyMap,
      [proxy.tag]: result.delay ?? null,
    };
  }

  #addToTesting(proxyTag: string) {
    const next = new Set(this.testingProxyIds);
    next.add(proxyTag);
    this.testingProxyIds = next;
  }

  #removeFromTesting(proxyTag: string) {
    const next = new Set(this.testingProxyIds);
    next.delete(proxyTag);
    this.testingProxyIds = next;
  }
}

export const latencyStore = new LatencyStore();
