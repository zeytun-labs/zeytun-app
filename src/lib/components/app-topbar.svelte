<script lang="ts">
  import { Toggle } from "$lib/components/ui/toggle";
  import { Separator } from "./ui/separator";
  import { profileStore } from "$lib/stores/profile.svelte";
  import { updaterStore } from "$lib/stores/updater.svelte";
  import {
    coreSetInboundMode,
    coreSetSystemProxy,
    coreRestart,
  } from "$lib/core/api";
  import { onMount } from "svelte";
  import { Spinner } from "$lib/components/ui/spinner/index.js";
  import * as Tooltip from "$lib/components/ui/tooltip/index.js";
  import NotificationsBell from "$lib/components/notifications-bell.svelte";
  import Logo from "./logo.svelte";
  import { LetterSwap } from "$lib/components/fancy/letter-swap";
  import { page } from "$app/state";
  import { HugeiconsIcon } from "@hugeicons/svelte";
  import { Rotate02Icon, SystemUpdate01Icon } from "@hugeicons/core-free-icons";
  import { toast } from "svelte-sonner";
  import { fly, slide, scale, fade } from "svelte/transition";
  import { errorMessage } from "$lib/errors";

  const titles: Record<string, string> = {
    "/": "Home",
    "/control-center": "Control Center",
    "/process": "Process",
    "/policy": "Policy",
    "/rule": "Rule",
    "/logs": "Logs",
    "/tools": "Tools",
    "/dns": "DNS",
    "/inspector": "Traffic Monitor",
  };

  const pageTitle = $derived.by(() => {
    const path = page.url.pathname;
    if (titles[path]) return titles[path];
    const hit = Object.keys(titles).find(
      (k) => k !== "/" && path.startsWith(k),
    );
    return hit ? titles[hit] : "Zeytun";
  });

  const isSystemProxyOn = $derived(
    profileStore.profile?.local_proxy.system_proxy ?? false,
  );

  const isTunModeOn = $derived(
    profileStore.profile?.local_proxy.mode === "tun",
  );

  async function toggleSystemProxy() {
    try {
      await profileStore.toggleNetwork("systemProxy", () =>
        coreSetSystemProxy(!isSystemProxyOn),
      );
    } catch (err) {
      console.error("Failed to toggle System Proxy:", err);
      toast.error("Failed to toggle System Proxy", {
        description: errorMessage(err),
      });
    }
  }

  async function toggleTunMode() {
    try {
      await profileStore.toggleNetwork("tun", () =>
        coreSetInboundMode(isTunModeOn ? "mixed" : "tun"),
      );
    } catch (err) {
      console.error("Failed to toggle TUN Mode:", err);
      toast.error("Failed to toggle TUN Mode", {
        description: errorMessage(err),
      });
    }
  }

  // While restarting the core cycles stopped→connecting→running; hold the
  // spinner across the whole cycle so the light never flashes red mid-restart.
  let restarting = $state(false);
  $effect(() => {
    if (restarting && profileStore.coreStatus === "running") restarting = false;
  });

  async function handleRestartCore() {
    restarting = true;
    // Safety: never let the spinner hang if the core never reaches "running".
    const guard = setTimeout(() => (restarting = false), 15000);
    try {
      await coreRestart();
      await profileStore.clearPendingRestart();
      toast.success("Core restarted successfully");
    } catch (err) {
      restarting = false;
      clearTimeout(guard);
      console.error("Failed to restart core:", err);
      toast.error("Failed to restart core", { description: errorMessage(err) });
    }
  }
</script>

<!-- Floating glass bar; sticky so content can scroll under with blur. -->
<header
  class="pointer-events-none sticky top-0 z-40 shrink-0 pl-2 pr-6 pt-4 pb-2"
  data-tauri-drag-region
>
  <div
    class="pointer-events-auto relative flex h-11 items-center gap-2 rounded-full border border-border/40 bg-card/70 px-3 shadow-lg backdrop-blur-xl backdrop-saturate-150"
    data-tauri-drag-region
  >
    <!-- Left: wordmark · page -->
    <div class="flex min-w-0 items-center gap-2 ps-1" data-tauri-drag-region>
      <Logo class="h-4! w-auto! text-lime-100" />
      <span
        class="text-muted-foreground/50 select-none text-sm"
        aria-hidden="true">·</span
      >
      <!-- Keying remounts LetterSwap on title change so the letter-swap
           replays on every page switch. -->
      {#key pageTitle}
        <LetterSwap
          label={pageTitle}
          staggerDuration={0}
          class="text-sm leading-5 text-muted-foreground"
        />
      {/key}
    </div>

    <div class="flex-1" data-tauri-drag-region></div>

    <!-- Right controls -->
    <div class="flex shrink-0 items-center gap-1">
      <NotificationsBell />

      <Separator orientation="vertical" class="mx-1 h-4!" />

      <Toggle
        aria-label="Toggle System Proxy"
        pressed={isSystemProxyOn}
        disabled={profileStore.loading || profileStore.networkToggle !== null}
        onclick={toggleSystemProxy}
        class="data-[state=on]:[&_span.size-2]:bg-primary h-8 gap-2 text-muted-foreground data-[state=on]:text-foreground hover:bg-foreground/5 disabled:opacity-100"
      >
        <span class="flex size-3 items-center justify-center">
          {#if profileStore.networkToggle === "systemProxy"}
            <Spinner class="size-3 border-muted-foreground/30 border-t-foreground" aria-label="Changing System Proxy" />
          {:else}
            <span class="size-2 rounded-full bg-muted-foreground/50 duration-150"></span>
          {/if}
        </span>
        System Proxy
      </Toggle>
      <Toggle
        aria-label="Toggle TUN Mode"
        pressed={isTunModeOn}
        disabled={profileStore.loading || profileStore.networkToggle !== null}
        onclick={toggleTunMode}
        class="data-[state=on]:[&_span.size-2]:bg-primary h-8 gap-2 text-muted-foreground data-[state=on]:text-foreground hover:bg-foreground/5 disabled:opacity-100"
      >
        <span class="flex size-3 items-center justify-center">
          {#if profileStore.networkToggle === "tun"}
            <Spinner class="size-3 border-muted-foreground/30 border-t-foreground" aria-label="Changing TUN Mode" />
          {:else}
            <span class="size-2 rounded-full bg-muted-foreground/50 duration-150"></span>
          {/if}
        </span>
        TUN Mode
      </Toggle>

      {#if profileStore.pendingRestart && profileStore.isRunning}
        <div
          transition:slide={{ axis: "x", duration: 250 }}
          class="flex items-center"
        >
          <div
            in:fly={{ y: -5, opacity: 0, delay: 100, duration: 200 }}
            out:fly={{ y: -5, opacity: 0, duration: 150 }}
          >
            <Tooltip.Root>
              <Tooltip.Trigger
                class="ms-1 flex h-8 items-center gap-1.5 rounded-full bg-primary/10 px-3 text-primary text-sm font-medium hover:bg-primary/30 transition-colors whitespace-nowrap overflow-hidden"
                onclick={handleRestartCore}
              >
                <HugeiconsIcon icon={Rotate02Icon} class="size-4" />
                Restart Core
              </Tooltip.Trigger>
              <Tooltip.Content side="bottom">
                <p>Restart core to apply pending changes</p>
              </Tooltip.Content>
            </Tooltip.Root>
          </div>
        </div>
      {/if}

      {#if updaterStore.ready}
        <div
          transition:slide={{ axis: "x", duration: 250 }}
          class="flex items-center"
        >
          <div
            in:fly={{ y: -5, opacity: 0, delay: 100, duration: 200 }}
            out:fly={{ y: -5, opacity: 0, duration: 150 }}
          >
            <Tooltip.Root>
              <Tooltip.Trigger
                class="ms-1 flex h-8 items-center gap-1.5 rounded-full bg-primary/10 px-3 text-primary text-sm font-medium hover:bg-primary/30 transition-colors whitespace-nowrap overflow-hidden"
                onclick={() => void updaterStore.install()}
              >
                <HugeiconsIcon icon={SystemUpdate01Icon} class="size-4" />
                Update
              </Tooltip.Trigger>
              <Tooltip.Content side="bottom">
                <p>Install Zeytun {updaterStore.version} and restart</p>
              </Tooltip.Content>
            </Tooltip.Root>
          </div>
        </div>
      {/if}

      <Tooltip.Root>
        <Tooltip.Trigger>
          <div class="relative flex h-8 w-6 items-center justify-center">
            {#if restarting}
              <Spinner class="size-4" />
            {:else if profileStore.coreStatus === "running"}
              <span class="block size-2 rounded-full bg-lime-500 duration-150"
              ></span>
            {:else if profileStore.coreStatus === "stopped"}
              <span class="block size-2 rounded-full bg-red-500 duration-150"
              ></span>
            {:else}
              <Spinner class="size-4" />
            {/if}
          </div>
        </Tooltip.Trigger>
        <Tooltip.Content>
          {#if restarting}
            <p>Restarting…</p>
          {:else if profileStore.coreStatus === "running"}
            <p>Core Started</p>
          {:else if profileStore.coreStatus === "stopped"}
            <p>Core Stopped!</p>
          {:else}
            <p>Connecting…</p>
          {/if}
        </Tooltip.Content>
      </Tooltip.Root>
    </div>
  </div>
</header>
