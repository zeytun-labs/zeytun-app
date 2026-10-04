<script lang="ts">
  import { onMount } from "svelte";
  import { fade, scale } from "svelte/transition";
  import { cubicIn, cubicOut } from "svelte/easing";
  import { listen, type UnlistenFn } from "@tauri-apps/api/event";
  import {
    coreDecideConnectionAsk,
    coreOpenConnectionAskWindow,
  } from "$lib/core/api";
  import { profileStore } from "$lib/stores/profile.svelte";
  import { errorMessage } from "$lib/errors";
  import { Button } from "$lib/components/ui/button/index.js";
  import { Switch } from "$lib/components/ui/switch/index.js";
  import { Label } from "$lib/components/ui/label/index.js";
  import * as Select from "$lib/components/ui/select/index.js";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import ProcessIcon from "../../routes/process/components/process-icon.svelte";
  import { ShineBorder } from "./magic/shine-border";

  // Match out: duration so hide doesn't clip the card.
  const HIDE_MS = 180;

  type AskPayload = {
    id: string;
    ts_ms: number;
    scope: number;
    code: string;
    severity: number;
    title: string;
    message: string;
    attrs: Record<string, string>;
  };

  type PendingAsk = {
    id: string;
    process_name: string;
    process_path: string;
    process_bundle?: string;
    dest_host: string;
    dest_port: string;
    group_by: string;
    group_key?: string;
    timeout_ms: number;
    expiresAt: number;
  };

  let queue = $state<PendingAsk[]>([
    // {
    //   id: "test",
    //   process_name: "Google Chrome",
    //   process_path:
    //     "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
    //   dest_host: "142.250.181.206",
    //   dest_port: "443",
    //   group_by: "process",
    //   timeout_ms: 60000,
    //   expiresAt: Date.now() + 2000000,
    // },
  ]);
  let current = $derived(queue[0] ?? null);
  let outbound = $state("direct");
  let remember = $state(true);
  let busy = $state(false);
  let unlistenAsk: UnlistenFn | null = null;
  let unlistenStatus: UnlistenFn | null = null;
  let hideTimer: ReturnType<typeof setTimeout> | null = null;

  const isAskWindow = () =>
    window.location.pathname.startsWith("/connection-ask");

  async function hideAskWindow() {
    if (!isAskWindow()) return;
    const win = getCurrentWindow();
    // Verify + retry. A swallowed hide (missing capability, transient failure)
    // used to leave the window on screen with an empty queue — a source of the
    // "glow visible but no dialog" state. Confirm it actually hid.
    for (let attempt = 0; attempt < 3; attempt++) {
      try {
        await win.hide();
        if (!(await win.isVisible())) return;
      } catch (e) {
        console.error("connection-ask: hide failed", e);
      }
      await new Promise((r) => setTimeout(r, 50));
    }
  }

  function scheduleHide() {
    if (hideTimer) return;
    hideTimer = setTimeout(() => {
      hideTimer = null;
      void hideAskWindow();
    }, HIDE_MS);
  }

  function cancelHide() {
    if (!hideTimer) return;
    clearTimeout(hideTimer);
    hideTimer = null;
  }

  async function showAskWindow() {
    if (!isAskWindow()) return;
    const win = getCurrentWindow();
    try {
      await win.show();
      await win.setFocus();
    } catch (e) {
      console.error("connection-ask: show failed", e);
    }
  }

  function resetDraft() {
    remember =
      profileStore.profile?.local_proxy?.connection_ask?.remember_default ??
      true;
    outbound = "direct";
  }

  // Core SIGHUP/reload drops all pending ids — UI queue is dead after that.
  function invalidateQueue() {
    if (queue.length === 0) return;
    queue = [];
    busy = false;
  }

  // ── Single visibility invariant ────────────────────────────────────────
  // The OS window is shown IFF the queue is non-empty, and the dialog is only
  // rendered AFTER the window is actually on screen. A hidden WKWebView
  // throttles requestAnimationFrame, which freezes the card's opacity/scale
  // enter transition at 0 while the glow's CSS keyframes keep animating — that
  // is exactly the "halo without dialog" symptom. Showing first, then flipping
  // `rendered`, guarantees the enter transition runs against a live webview.
  // Emptying the queue always drives a verified hide, so the window can never
  // linger empty on screen. Every queue mutation re-runs this effect.
  let rendered = $state(false);
  let syncGen = 0;

  async function syncWindow(hasItems: boolean) {
    const gen = ++syncGen;
    if (hasItems) {
      cancelHide();
      await showAskWindow();
      if (gen !== syncGen) return; // a newer sync superseded this one
      rendered = true;
    } else {
      rendered = false; // let the leave transition play, then hide
      scheduleHide();
    }
  }

  $effect(() => {
    void syncWindow(queue.length > 0);
  });

  const options = $derived.by(() => {
    const builtin = [{ tag: "direct", name: "Direct" }];
    const policies =
      profileStore.profile?.policies.map((p) => ({
        tag: p.tag,
        name: p.name || p.tag,
      })) ?? [];
    const proxies =
      profileStore.profile?.proxies
        .filter((p) => p.enabled)
        .map((p) => ({ tag: p.tag, name: p.title || p.tag })) ?? [];
    // Root policy leaks as "ROOT_POLICY" or "root-policy" — drop case-insensitively.
    return [...builtin, ...policies, ...proxies].filter(
      (o) => o.tag.toLowerCase().replaceAll("-", "_") !== "root_policy",
    );
  });

  function enqueue(p: AskPayload) {
    if (p.code !== "CONNECTION_ASK" && p.scope !== 5) return;
    const id = p.attrs?.id || p.id;
    if (!id) return;

    if (!isAskWindow()) {
      void coreOpenConnectionAskWindow();
    }

    if (queue.some((q) => q.id === id)) return;

    const gk = p.attrs?.group_key;
    // Guard against a 0/NaN timeout: "0" is truthy so `|| 60000` misses it,
    // and Number(undefined) is NaN — either makes expiresAt <= now, so the
    // expiry sweep drops the item instantly and the window shows empty.
    const parsed = Number(p.attrs?.timeout_ms);
    const timeout = Number.isFinite(parsed) && parsed > 0 ? parsed : 60000;
    const item: PendingAsk = {
      id,
      process_name: p.attrs?.process_name || "Unknown",
      process_path: p.attrs?.process_path || "",
      process_bundle: p.attrs?.process_bundle,
      dest_host: p.attrs?.dest_host || p.attrs?.dest || "",
      dest_port: p.attrs?.dest_port || "",
      group_by: p.attrs?.group_by || "process",
      group_key: gk,
      timeout_ms: timeout,
      expiresAt: Date.now() + timeout,
    };

    // Same process/group after core reload: replace stale id, don't drop live ask.
    // The visibility $effect reacts to the queue mutation and shows the window.
    if (gk) {
      const idx = queue.findIndex((q) => q.group_key === gk);
      if (idx >= 0) {
        const next = [...queue];
        next[idx] = item;
        queue = next;
        if (idx === 0) resetDraft();
        return;
      }
    }

    queue = [...queue, item];
    if (queue.length === 1) resetDraft();
  }

  onMount(() => {
    void (async () => {
      unlistenAsk = await listen<AskPayload>("core-notification", (ev) => {
        const p = ev.payload;
        // After rule delete / SIGHUP, router is new — pending map empty.
        if (p.code === "SERVICE_STARTED" || p.code === "SERVICE_STARTING") {
          invalidateQueue();
          return;
        }
        enqueue(p);
      });
      unlistenStatus = await listen<{ status?: string }>(
        "core-status-changed",
        (ev) => {
          const st = ev.payload?.status;
          if (st === "starting" || st === "stopping" || st === "idle") {
            invalidateQueue();
          }
        },
      );
    })();

    const t = setInterval(() => {
      const now = Date.now();
      if (queue.some((q) => q.expiresAt <= now)) {
        queue = queue.filter((q) => q.expiresAt > now);
      }
    }, 1000);

    return () => {
      unlistenAsk?.();
      unlistenStatus?.();
      clearInterval(t);
      cancelHide();
    };
  });

  async function decide(reject: boolean, useFinal = false) {
    if (!current || busy) return;
    const decidingId = current.id;
    busy = true;
    try {
      const out = reject ? "block" : useFinal ? "" : outbound;
      await coreDecideConnectionAsk({
        id: decidingId,
        outbound: out === "block" ? "" : out,
        reject: reject || out === "block",
        remember: remember && !useFinal,
        process_path: current.process_path,
        process_bundle: current.process_bundle,
        dest_host: current.dest_host,
        group_by: current.group_by,
      });
      if (remember && !reject && !useFinal) {
        await profileStore.refresh();
      }
      queue = queue.filter((q) => q.id !== decidingId);
      if (queue[0]) resetDraft();
      // empty → $effect scheduleHide after out transition
    } catch (e) {
      const msg = errorMessage(e);
      console.error("connection-ask decide failed", msg, e);
      // Stale id after reload: drop and wait for fresh CONNECTION_ASK.
      if (/pending connection not found|not found/i.test(msg)) {
        queue = queue.filter((q) => q.id !== decidingId);
        if (queue[0]) resetDraft();
      }
    } finally {
      busy = false;
    }
  }
</script>

{#if current && rendered}
  <div class="relative m-4 w-full h-full flex justify-center items-center">
    <!-- ambient glow behind the card (lime + orange, breathing).
         Absolute layer overhanging the card so NO layer edge falls inside the
         window (WKWebView-safe: no mask, no fixed). Pixel gradients fade out
         completely before every window edge -> no clip line possible. -->
    <div
      aria-hidden="true"
      class="pointer-events-none absolute left-0 right-0 top-0 bottom-0"
      in:fade={{ duration: 220 }}
      out:fade={{ duration: HIDE_MS }}
    >
      <div class="glow-blob glow-lime motion-safe:animate-glow-lime"></div>
      <div class="glow-blob glow-orange motion-safe:animate-glow-orange"></div>
    </div>

    <div
      class="min-w-md bg-popover bg-no-repeat bg-size-[100%_150px] ring-foreground/5 dark:ring-foreground/10 grid rounded-4xl text-sm shadow-xl ring-1 outline-none bg-linear-to-r from-yellow-400 to-lime-500 to-40% relative overflow-hidden"
      data-tauri-drag-region
      in:scale={{ duration: 220, start: 0.92, opacity: 0, easing: cubicOut }}
      out:scale={{
        duration: HIDE_MS,
        start: 0.92,
        opacity: 0,
        easing: cubicIn,
      }}
    >
      <ShineBorder shineColor={["#A6FE8F", "#FFBE7B"]} />

      <div class="text-center py-1.5 text-popover font-bold">
        NEW CONNECTION
      </div>
      <div
        class="grid gap-6 bg-popover text-popover-foreground p-6 rounded-[24px]"
      >
        <div class="flex items-center gap-3">
          <ProcessIcon
            name={current.process_name}
            path={current.process_path}
            class="size-10 shrink-0 rounded-lg"
          />
          <div class="flex min-w-0 flex-col">
            <h2
              class="text-base font-semibold truncate"
              title={current.process_path}
            >
              {current.process_name}
            </h2>
            <p class="text-muted-foreground text-sm">
              pick outbound or wait for FINAL timeout.
            </p>
          </div>
        </div>

        <div class="space-y-3 text-sm">
          <div class="flex gap-2">
            <span class="text-muted-foreground">Destination</span>
            <span class="truncate"
              >{current.dest_host}{current.dest_port
                ? `:${current.dest_port}`
                : ""}</span
            >
          </div>

          <div class="space-y-2">
            <Label>Action</Label>
            <Select.Root
              type="single"
              value={outbound}
              onValueChange={(v) => {
                if (v) outbound = v;
              }}
            >
              <Select.Trigger class="w-full">
                {options.find((o) => o.tag === outbound)?.name || outbound}
              </Select.Trigger>
              <Select.Content>
                <Select.Group>
                  {#each options as opt}
                    <Select.Item value={opt.tag} label={opt.name}
                      >{opt.name}</Select.Item
                    >
                  {/each}
                </Select.Group>
              </Select.Content>
            </Select.Root>
          </div>

          <div class="flex items-center gap-2 pt-2">
            <Switch id="ask-remember" bind:checked={remember} />
            <Label for="ask-remember" class="font-normal"
              >Remember (add process rule)</Label
            >
          </div>
        </div>

        <div class="flex gap-2 flex-row justify-end">
          <Button variant="outline" disabled={busy} onclick={() => decide(true)}
            >Block</Button
          >
          <Button
            variant="secondary"
            disabled={busy}
            onclick={() => decide(false, true)}>Use FINAL</Button
          >
          <Button class="flex-1" disabled={busy} onclick={() => decide(false)}
            >Allow</Button
          >
        </div>
      </div>
    </div>
  </div>
{/if}

<style>
  .glow-blob {
    position: absolute;
    inset: 0;
    will-change: transform, opacity;
  }

  /* Two breathing blooms around the card (lime top-left, orange bottom-right).
     Window is 680x650 so the 300px blob fade circles stay fully inside the
     window at the animation extremes. transform-origin pins breathing to the
     gradient center. */
  .glow-lime {
    background: radial-gradient(
      40% 40% at 33% 36%,
      rgba(132, 204, 22, 0.6),
      transparent 75%
    );
    transform-origin: 38% 40%;
  }

  .glow-orange {
    background: radial-gradient(
      40% 40% at 67% 64%,
      rgba(245, 158, 11, 0.6),
      transparent 75%
    );
    transform-origin: 60% 58%;
  }
</style>
