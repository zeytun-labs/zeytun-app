<script lang="ts">
  import AppSidebar from "$lib/components/app-sidebar.svelte";
  import AppTopbar from "$lib/components/app-topbar.svelte";
  import { themePreference } from "$lib/theme.svelte.js";
  import { settingsStore } from "$lib/stores/settings.svelte";
  import { ScrollArea } from "$lib/components/ui/scroll-area";
  import { Button } from "$lib/components/ui/button";
  import { onMount } from "svelte";
  import { fly } from "svelte/transition";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import * as Tooltip from "$lib/components/ui/tooltip/index.js";
  import { coreSetMode, coreStart } from "$lib/core/api";
  import { profileStore } from "$lib/stores/profile.svelte";
  import { logsStore } from "$lib/stores/logs.svelte";
  import { notificationStore } from "$lib/stores/notifications.svelte";
  import { updaterStore } from "$lib/stores/updater.svelte";
  import { geoipStore } from "$lib/stores/geoip.svelte";
  import { page } from "$app/state";
  import { goto } from "$app/navigation";
  import { listen, type UnlistenFn } from "@tauri-apps/api/event";
  import { Spinner } from "$lib/components/ui/spinner";

  import "./layout.css";
  import { HugeiconsIcon } from "@hugeicons/svelte";
  import { Alert01Icon } from "@hugeicons/core-free-icons";

  import SettingsDialog from "$lib/components/settings/settings-dialog.svelte";
  import CriticalBanner from "$lib/components/critical-banner.svelte";
  import { Toaster } from "$lib/components/ui/sonner/index.js";

  const { children } = $props();

  // The connection-ask window has its own self-contained layout — skip the
  // dashboard shell AND the core startup sequence for it. Inspector is a
  // regular route inside the main window.
  const bare = $derived(page.url.pathname.startsWith("/connection-ask"));
  const isConnectionAsk = $derived(
    page.url.pathname.startsWith("/connection-ask"),
  );

  $effect(() => {
    if (typeof document === "undefined") return;
    document.documentElement.classList.toggle(
      "connection-ask",
      isConnectionAsk,
    );
  });

  let isAppInitializing = $state(true);
  let startupError = $state<string | null>(null);
  let settingsOpen = $state(false);
  let unlistenOpenInspector: UnlistenFn | null = null;

  // Show the same full-page loading overlay while a profile switch is in flight
  // (the backend regenerates config + restarts the core, which briefly freezes
  // the UI). Reuses the startup screen so the transition feels intentional.
  const showLoadingOverlay = $derived(
    isAppInitializing || profileStore.switching,
  );

  async function runStartupSequence() {
    startupError = null;
    try {
      // Direct mode on every launch, then bring the core up.
      // lifecycle.start() PATCHes the clash API mode immediately after boot
      // to override any stale cached mode, so the order here is safe.
      await coreSetMode("direct");
      await coreStart();
      // Populate the profile registry so the sidebar profile switcher is
      // available on every page, not just Policy/Inspector.
      await profileStore.load();
      // Both background checks need saved settings before reading their toggles.
      await settingsStore.init();
      isAppInitializing = false;
      // Updater needs the profile and never blocks startup.
      updaterStore.startAuto();
      // GeoIP refresh is fire-and-forget (a slow CDN must never block startup).
      if (settingsStore.settings.autoUpdateGeoIp) {
        void geoipStore.autoUpdate();
      }
    } catch (err) {
      startupError = errorMessage(err);
    }
  }

  // Tauri command rejections arrive as the serialized CommandError
  // (`{ kind, message }`), not an Error instance — unwrap it for display.
  function errorMessage(err: unknown): string {
    if (typeof err === "string") return err;
    if (err && typeof err === "object" && "message" in err) {
      const m = (err as { message: unknown }).message;
      if (typeof m === "string") return m;
    }
    if (err instanceof Error) return err.message;
    return "Unknown startup error";
  }

  onMount(() => {
    themePreference.init();
    settingsStore.init();
    // Register backend sync listeners before the core boots so we don't miss the
    // daemon's first status transition.
    void profileStore.startListening();
    // Start listening to Core logs and retaining them in the background
    void logsStore.init();
    void notificationStore.init();
    document.addEventListener("open-settings", handleOpenSettings);

    // If bare (e.g. connection-ask), just keep listening but do NOT run core startup.
    if (bare) {
      return () => {
        profileStore.stopListening();
        logsStore.stopListening();
        notificationStore.stopListening();
        document.removeEventListener("open-settings", handleOpenSettings);
      };
    }

    // Tray "Open Traffic Monitor" → navigate this window to the Inspector route.
    void listen("open-inspector", () => void goto("/inspector")).then(
      (fn) => (unlistenOpenInspector = fn),
    );

    // The loading UI is now rendered — reveal the window (kills the white flash)
    // and kick off the core startup. show() is fire-and-forget.
    void getCurrentWindow().show();
    void runStartupSequence();

    return () => {
      profileStore.stopListening();
      logsStore.stopListening();
      notificationStore.stopListening();
      unlistenOpenInspector?.();
      document.removeEventListener("open-settings", handleOpenSettings);
    };
  });

  let settingsTab = $state("general");

  function handleOpenSettings(e?: Event) {
    const custom = e as CustomEvent<{ tab?: string }>;
    settingsTab = custom?.detail?.tab ?? "general";
    settingsOpen = true;
  }
</script>

{#if bare}
  {@render children()}
{:else if showLoadingOverlay}
  <div
    class="bg-background text-foreground flex h-screen flex-col items-center justify-center gap-6"
    data-tauri-drag-region
  >
    {#if startupError && !profileStore.switching}
      <div class="flex max-w-md flex-col items-center gap-4 text-center">
        <div
          class="flex size-12 items-center justify-center rounded-full bg-destructive/10 text-destructive"
        >
          <HugeiconsIcon icon={Alert01Icon} />
        </div>
        <div class="flex flex-col gap-1">
          <h1 class="text-lg font-semibold">Startup Error</h1>
          <p class="text-muted-foreground text-sm wrap-break-word">
            {startupError}
          </p>
        </div>
        <Button onclick={() => void runStartupSequence()}>Retry</Button>
      </div>
    {:else}
      <div class="flex flex-col items-center gap-4">
        <Spinner class="size-6" />
        <p class="text-muted-foreground text-sm font-medium">
          {profileStore.switching
            ? "Switching profile…"
            : "Starting Zeytun Core…"}
        </p>
      </div>
    {/if}
  </div>
{:else}
  <Tooltip.Provider>
    <div
      class="relative flex h-screen flex-row overflow-hidden"
      data-tauri-drag-region
    >
      <div class="w-24">
        <AppSidebar />
      </div>
      <!-- pl leaves room for fixed icon rail (w-14 + start-3 gutter) -->
      <div
        class="relative flex min-h-0 min-w-0 flex-1 flex-col"
        data-tauri-drag-region
      >
        <CriticalBanner />
        <div class="relative flex min-h-0 flex-1 flex-col pr-2">
          <!-- sticky glass topbar sits over scroll so blur catches content -->
          <div class="absolute inset-x-0 top-0 z-40">
            <AppTopbar />
          </div>
          <ScrollArea class="h-0 min-h-0 flex-1">
            <!-- keying on the pathname remounts the page on route change, so
                 the in:fly transition plays on every sidebar switch -->
            {#key page.url.pathname}
              <main
                in:fly={{ y: 16, duration: 250 }}
                class="flex flex-col pl-2 pt-20 pr-4 *:min-w-0"
                data-tauri-drag-region
              >
                {@render children()}
              </main>
            {/key}
          </ScrollArea>
        </div>
      </div>
    </div>
  </Tooltip.Provider>
  <SettingsDialog bind:open={settingsOpen} initialTab={settingsTab} />
  <Toaster
    position="bottom-right"
    theme={themePreference.value === "system"
      ? "system"
      : themePreference.value}
  />
{/if}
