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
  import { coreSetMode, coreStart, crashDumpListUnseen, crashDumpRead, crashDumpDismiss } from "$lib/core/api";
  import { toast } from "svelte-sonner";
  import { writeText } from "@tauri-apps/plugin-clipboard-manager";
  import { profileStore } from "$lib/stores/profile.svelte";
  import { logsStore } from "$lib/stores/logs.svelte";
  import { notificationStore, SCOPE_SYSTEM } from "$lib/stores/notifications.svelte";
  import { updaterStore } from "$lib/stores/updater.svelte";
  import { geoipStore } from "$lib/stores/geoip.svelte";
  import { page } from "$app/state";
  import { goto } from "$app/navigation";
  import { listen, type UnlistenFn } from "@tauri-apps/api/event";
  import { Spinner } from "$lib/components/ui/spinner";

  import "./layout.css";
  import { HugeiconsIcon } from "@hugeicons/svelte";
  import {
    Alert01Icon,
    RefreshIcon,
    Settings02Icon,
    Copy01Icon,
    Tick01Icon,
  } from "@hugeicons/core-free-icons";
  import LogoLoader from "$lib/components/logo-loader.svelte";

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
  let startupProgress = $state(0);
  let copied = $state(false);
  let settingsOpen = $state(false);
  let unlistenOpenInspector: UnlistenFn | null = null;

  async function copyStartupError() {
    if (!startupError) return;
    try {
      await writeText(startupError);
      copied = true;
      toast.success("Error copied to clipboard");
      setTimeout(() => {
        copied = false;
      }, 2000);
    } catch {
      toast.error("Failed to copy error");
    }
  }

  async function runStartupSequence() {
    startupError = null;
    startupProgress = 15;
    try {
      // Load saved settings and profile registry first (local SQLite, independent of core).
      await settingsStore.init();
      startupProgress = 40;

      await profileStore.load();
      startupProgress = 65;

      // Direct mode on every launch, then bring the core up.
      // lifecycle.start() PATCHes the clash API mode immediately after boot
      // to override any stale cached mode, so the order here is safe.
      await coreSetMode("direct");
      startupProgress = 80;

      await coreStart();
      await profileStore.refresh();
      startupProgress = 100;

      // Brief gentle transition to let 100% register before unmounting
      await new Promise((r) => setTimeout(r, 150));

      isAppInitializing = false;
      void checkCrashDumps();
      // Updater needs the profile and never blocks startup.
      updaterStore.startAuto();
      // GeoIP refresh is fire-and-forget (a slow CDN must never block startup).
      if (settingsStore.settings.autoUpdateGeoIp) {
        void geoipStore.autoUpdate();
      }
    } catch (err) {
      startupError = errorMessage(err);
      try {
        await settingsStore.init();
        await profileStore.load();
      } catch {
        // ignore fallback failure
      }
    }
  }

  async function checkCrashDumps() {
    try {
      const dumps = await crashDumpListUnseen();
      if (!dumps || dumps.length === 0) return;

      const latest = dumps[0];
      const content = await crashDumpRead(latest).catch(() => null);

      notificationStore.pushNotification({
        title: "Previous session crashed",
        message:
          dumps.length > 1
            ? `${dumps.length} crash reports found`
            : "1 crash report found",
        severity: 2,
        scope: SCOPE_SYSTEM,
        code: "CRASH_DUMP",
      });

      toast.error("Zeytun crashed in the previous session", {
        description:
          dumps.length > 1
            ? `${dumps.length} crash reports found`
            : "1 crash report found",
        duration: 15000,
        action: content
          ? {
              label: "Copy Report",
              onClick: async () => {
                try {
                  await writeText(content);
                  toast.success("Crash report copied to clipboard");
                } catch {
                  toast.error("Failed to copy");
                }
              },
            }
          : undefined,
        onDismiss: () => {
          void crashDumpDismiss();
        },
      });
    } catch (err) {
      console.error("Failed to check crash dumps:", err);
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
    // Tauri opens index.html by default; SvelteKit's Home route is /.
    if (window.location.pathname === "/index.html") {
      void goto("/", { replaceState: true });
    }
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

    let unlistenWake: UnlistenFn | undefined;
    void listen("system-wake", async () => {
      try {
        await profileStore.refresh();
        void updaterStore.check(true);
      } catch (err) {
        console.error("Failed to refresh on system wake:", err);
      }
    }).then((fn) => (unlistenWake = fn));

    const handleVisibilityChange = () => {
      if (document.visibilityState === "visible") {
        void profileStore.refresh();
      }
    };
    document.addEventListener("visibilitychange", handleVisibilityChange);

    // The loading UI is now rendered — reveal the window (kills the white flash)
    // and kick off the core startup. show() is fire-and-forget.
    void getCurrentWindow().show();
    void runStartupSequence();

    return () => {
      profileStore.stopListening();
      logsStore.stopListening();
      notificationStore.stopListening();
      unlistenOpenInspector?.();
      unlistenWake?.();
      document.removeEventListener("open-settings", handleOpenSettings);
      document.removeEventListener("visibilitychange", handleVisibilityChange);
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
{:else}
  {#if isAppInitializing}
    <div
      class="bg-background text-foreground relative flex h-screen flex-col items-center justify-center p-6 select-none"
      data-tauri-drag-region
    >
      {#if startupError}
        <div
          class="bg-card/75 border-border/80 shadow-2xl relative flex w-full max-w-md flex-col items-center gap-5 rounded-2xl border p-6 text-center backdrop-blur-xl animate-in fade-in zoom-in-95 duration-200"
        >
          <div
            class="bg-destructive/10 text-destructive flex size-12 items-center justify-center rounded-full"
          >
            <HugeiconsIcon icon={Alert01Icon} class="size-6" />
          </div>

          <div class="flex flex-col gap-1.5">
            <h1 class="text-base font-semibold text-foreground">Core Startup Failed</h1>
            <p class="text-muted-foreground text-xs leading-relaxed max-w-sm">
              Zeytun could not start the core proxy engine. You can retry, adjust your configuration, or continue in offline mode.
            </p>
          </div>

          <div class="w-full overflow-hidden rounded-xl border border-border/60 bg-muted/40 text-left">
            <div class="flex items-center justify-between border-b border-border/40 px-3 py-1.5 bg-muted/20">
              <span class="text-[11px] font-mono font-medium text-muted-foreground">Error Details</span>
              <Button
                variant="ghost"
                size="sm"
                class="h-6 gap-1 px-2 text-[11px] text-muted-foreground hover:text-foreground"
                onclick={copyStartupError}
              >
                <HugeiconsIcon icon={copied ? Tick01Icon : Copy01Icon} class="size-3 text-muted-foreground" />
                <span>{copied ? "Copied" : "Copy"}</span>
              </Button>
            </div>
            <pre class="max-h-28 overflow-y-auto p-3 font-mono text-xs text-foreground/80 whitespace-pre-wrap break-all select-text">{startupError}</pre>
          </div>

          <div class="flex w-full flex-col gap-2 pt-1">
            <div class="flex w-full gap-2">
              <Button
                class="flex-1 gap-2"
                onclick={() => void runStartupSequence()}
              >
                <HugeiconsIcon icon={RefreshIcon} class="size-4" />
                <span>Retry</span>
              </Button>

              <Button
                variant="outline"
                class="gap-2"
                onclick={() => { settingsOpen = true; }}
              >
                <HugeiconsIcon icon={Settings02Icon} class="size-4" />
                <span>Settings</span>
              </Button>
            </div>

            <Button
              variant="ghost"
              class="w-full text-xs text-muted-foreground hover:text-foreground h-8"
              onclick={() => { isAppInitializing = false; }}
            >
              Continue to App (Offline Mode)
            </Button>
          </div>
        </div>
      {:else}
        <div class="flex flex-col items-center gap-5 animate-in fade-in duration-300">
          <LogoLoader size={64} variant="shimmer" />
          <div class="w-16 h-[5px] rounded-full bg-border/60 overflow-hidden relative">
            <div
              class="h-full bg-primary rounded-full transition-all duration-300 ease-out shadow-[0_0_8px_var(--color-primary)]"
              style="width: {startupProgress}%;"
            ></div>
          </div>
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

    {#if profileStore.switching}
      <div
        class="bg-background/60 backdrop-blur-sm fixed inset-0 z-50 flex items-center justify-center transition-opacity animate-in fade-in duration-150"
        data-tauri-drag-region
      >
        <div
          class="bg-card/90 border-border/80 shadow-2xl flex items-center gap-3 rounded-2xl border px-5 py-3 text-foreground"
        >
          <Spinner class="size-4 text-primary" />
          <span class="text-xs font-medium">Switching profile…</span>
        </div>
      </div>
    {/if}
  {/if}

  <SettingsDialog bind:open={settingsOpen} initialTab={settingsTab} />
  <Toaster
    position="bottom-right"
    theme={themePreference.value === "system"
      ? "system"
      : themePreference.value}
  />
{/if}
