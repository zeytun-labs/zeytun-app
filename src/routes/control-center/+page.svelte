<script lang="ts">
  import * as Card from "$lib/components/ui/card";
  import { Switch } from "$lib/components/ui/switch";
  import { Spinner } from "$lib/components/ui/spinner/index.js";
  import * as Tooltip from "$lib/components/ui/tooltip";
  import { HugeiconsIcon } from "@hugeicons/svelte";
  import { InformationCircleIcon } from "@hugeicons/core-free-icons";
  import { profileStore } from "$lib/stores/profile.svelte";
  import { toast } from "svelte-sonner";
  import { errorMessage } from "$lib/errors";
  import {
    coreSetSystemProxy,
    coreSetInboundMode,
    coreSetAllowLan,
  } from "$lib/core/api";

  const isSystemProxyOn = $derived(
    profileStore.profile?.local_proxy.system_proxy === true,
  );

  const isTunModeOn = $derived(
    profileStore.profile?.local_proxy.mode === "tun",
  );

  const isLanAllowed = $derived(
    profileStore.profile?.local_proxy.listen === "0.0.0.0",
  );

  async function toggleSystemProxy(checked: boolean) {
    try {
      await profileStore.toggleNetwork("systemProxy", () =>
        coreSetSystemProxy(checked),
      );
    } catch (e) {
      console.error("Failed to toggle system proxy:", e);
      toast.error("Failed to toggle System Proxy", {
        description: errorMessage(e),
      });
    }
  }

  async function toggleTunMode(checked: boolean) {
    try {
      await profileStore.toggleNetwork("tun", () =>
        coreSetInboundMode(checked ? "tun" : "mixed"),
      );
    } catch (e) {
      console.error("Failed to toggle tun mode:", e);
      toast.error("Failed to toggle TUN Mode", {
        description: errorMessage(e),
      });
    }
  }

  async function toggleLanAccess(checked: boolean) {
    try {
      await coreSetAllowLan(checked);
      // Set pending restart flag instead of auto-refreshing
      profileStore.pendingRestart = true;
    } catch (e) {
      console.error("Failed to toggle LAN access:", e);
    }
  }
</script>

<div
  class="mx-auto w-full flex-1 gap-4 grid grid-cols-[auto_auto] min-h-0 min-w-0"
>
  <Card.Root class="bg-card/70 border-border/40 py-4 rounded-3xl">
    <Card.Header class="flex flex-row items-center justify-between">
      <div class="flex items-center gap-2">
        <Card.Title class="text-base font-semibold">System Proxy</Card.Title>
        <Tooltip.Provider>
          <Tooltip.Root>
            <Tooltip.Trigger>
              <HugeiconsIcon
                icon={InformationCircleIcon}
                class="size-4 text-muted-foreground hover:text-foreground transition-colors"
              />
            </Tooltip.Trigger>
            <Tooltip.Content side="bottom">
              <p class="max-w-xs">
                Handles traffic for applications that respect macOS proxy
                settings. It provides excellent compatibility and performance
                for most standard apps.
              </p>
            </Tooltip.Content>
          </Tooltip.Root>
        </Tooltip.Provider>
      </div>
      {#if profileStore.networkToggle === "systemProxy"}
        <Spinner aria-label="Changing System Proxy" />
      {/if}
      <Switch
        checked={isSystemProxyOn}
        onCheckedChange={toggleSystemProxy}
        disabled={profileStore.loading || profileStore.networkToggle !== null}
      />
    </Card.Header>
  </Card.Root>

  <Card.Root class="bg-card/70 border-border/40 py-4 rounded-3xl">
    <Card.Header class="flex flex-row items-center justify-between">
      <div class="flex items-center gap-2">
        <Card.Title class="text-base font-semibold">Tun Mode</Card.Title>
        <Tooltip.Provider>
          <Tooltip.Root>
            <Tooltip.Trigger>
              <HugeiconsIcon
                icon={InformationCircleIcon}
                class="size-4 text-muted-foreground hover:text-foreground transition-colors"
              />
            </Tooltip.Trigger>
            <Tooltip.Content side="bottom">
              <p class="max-w-xs">
                Captures traffic from apps that ignore system proxies by routing
                everything through a virtual interface. We highly recommend
                keeping this enabled and using Rules to precisely control which
                apps or domains go through the proxy.
              </p>
            </Tooltip.Content>
          </Tooltip.Root>
        </Tooltip.Provider>
      </div>
      {#if profileStore.networkToggle === "tun"}
        <Spinner aria-label="Changing TUN Mode" />
      {/if}
      <Switch
        checked={isTunModeOn}
        onCheckedChange={toggleTunMode}
        disabled={profileStore.loading || profileStore.networkToggle !== null}
      />
    </Card.Header>
  </Card.Root>

  <Card.Root class="bg-card/70 border-border/40 py-4 rounded-3xl">
    <Card.Header class="flex flex-row items-center justify-between">
      <div class="flex items-center gap-2">
        <Card.Title class="text-base font-semibold"
          >HTTP & SOCKS5 Proxy</Card.Title
        >
        <Tooltip.Provider>
          <Tooltip.Root>
            <Tooltip.Trigger>
              <HugeiconsIcon
                icon={InformationCircleIcon}
                class="size-4 text-muted-foreground hover:text-foreground transition-colors"
              />
            </Tooltip.Trigger>
            <Tooltip.Content side="bottom">
              <p class="max-w-xs">
                Allows Zeytun to function as a standard HTTP and SOCKS5 proxy
                server for other devices connected to your Local Area Network
                (LAN).
              </p>
            </Tooltip.Content>
          </Tooltip.Root>
        </Tooltip.Provider>
      </div>
      <Switch
        checked={isLanAllowed}
        onCheckedChange={toggleLanAccess}
        disabled={profileStore.loading}
      />
    </Card.Header>
  </Card.Root>
</div>
