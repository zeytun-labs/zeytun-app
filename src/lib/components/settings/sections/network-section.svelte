<script lang="ts">
  import { Input } from "$lib/components/ui/input/index.js";
  import { Button } from "$lib/components/ui/button/index.js";
  import { Switch } from "$lib/components/ui/switch/index.js";
  import * as Select from "$lib/components/ui/select/index.js";
  import { HugeiconsIcon } from "@hugeicons/svelte";
  import { GlobalRefreshIcon } from "@hugeicons/core-free-icons";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { type TrafficKind } from "$lib/core/api";
  import { geoipStore } from "$lib/stores/geoip.svelte";
  import type { SettingsDraft } from "../settings-draft.svelte";
  import SettingRow from "../setting-row.svelte";
  import SettingsSection from "../settings-section.svelte";
  import Spinner from "$lib/components/ui/spinner/spinner.svelte";

  let { draft }: { draft: SettingsDraft } = $props();

  /**
   * Route options. There are exactly two, matching what the app can actually do:
   * it dials either directly or through its own mixed inbound.
   *
   * A named policy is not dialable by the app — the core resolves those tags — so
   * listing policies would offer a choice that degrades to "Through Zeytun"
   * anyway. That is why the label is deliberately vague rather than naming a
   * policy.
   */
  const routeOptions = [
    { value: "direct", label: "Direct" },
    { value: "local_proxy", label: "Through Zeytun" },
  ];

  const trafficRows: { kind: TrafficKind; label: string; description: string }[] = [
    {
      kind: "app_update",
      label: "App Updates",
      description: "Route for checking and downloading Zeytun updates.",
    },
    {
      kind: "geoip",
      label: "GeoIP Database",
      description: "Route for downloading the country database.",
    },
    {
      kind: "subscription",
      label: "Subscriptions",
      description: "Route for fetching subscription profiles.",
    },
  ];

  function routeLabel(value: string): string {
    return routeOptions.find((o) => o.value === value)?.label ?? "Direct";
  }
</script>

<div class="space-y-6">
  <SettingsSection
    title="Network"
    description="Local proxy listener and connection behavior."
  >
    <SettingRow
      label="Listen Port"
      description="Local mixed proxy port. Changing it restarts the core."
      for="listenPort"
    >
      <div class="w-28 space-y-1.5">
        <Input
          id="listenPort"
          type="number"
          bind:value={draft.listenPort}
          aria-invalid={draft.errors.listenPort ? "true" : undefined}
          oninput={() => {
            if (draft.errors.listenPort) {
              draft.errors = { ...draft.errors, listenPort: undefined };
            }
          }}
        />
        {#if draft.errors.listenPort}
          <p class="text-xs leading-snug text-destructive">
            {draft.errors.listenPort}
          </p>
        {/if}
      </div>
    </SettingRow>

    <SettingRow
      label="Connection Ask"
      description="Prompt for outbound when a new connection matches no rules."
    >
      <Switch bind:checked={draft.settings.connectionAskEnabled} />
    </SettingRow>
  </SettingsSection>

  <SettingsSection
    title="Zeytun's Own Traffic"
    description="How the app reaches the internet for its own requests. Direct by default."
  >
    {#each trafficRows as row (row.kind)}
      <SettingRow label={row.label} description={row.description}>
        <div class="w-44 space-y-1.5">
          <Select.Root
            type="single"
            value={draft.routes[row.kind]}
            onValueChange={(v: string) => (draft.routes[row.kind] = v)}
          >
            <Select.Trigger class="h-9 w-full">
              {routeLabel(draft.routes[row.kind])}
            </Select.Trigger>
            <Select.Content>
              <Select.Group>
                {#each routeOptions as option (option.value)}
                  <Select.Item value={option.value} label={option.label} />
                {/each}
              </Select.Group>
            </Select.Content>
          </Select.Root>
        </div>
      </SettingRow>
    {/each}

    <SettingRow
      label="GeoIP Database"
      description="Country lookup for IP details and flags. Routing rules use separate rule sets."
    >
      <Button
        variant="secondary"
        class="gap-1.5"
        disabled={geoipStore.updating}
        onclick={() => void geoipStore.update()}
      >
        {#if geoipStore.updating}
          <Spinner class="size-4 border-secondary-foreground/30 border-t-secondary-foreground" />
        {:else}
          <HugeiconsIcon icon={GlobalRefreshIcon} />
        {/if}
        {geoipStore.updating ? "Updating…" : "Update Now"}
      </Button>
    </SettingRow>

    <SettingRow
      label="Automatic Update"
      description="Check for a newer country database when Zeytun starts."
      for="autoGeo"
    >
      <Switch id="autoGeo" bind:checked={draft.settings.autoUpdateGeoIp} />
    </SettingRow>
  </SettingsSection>

  <p class="px-1 text-xs text-muted-foreground">
    IP geolocation by <button
      type="button"
      class="rounded underline underline-offset-2 outline-none hover:text-foreground focus-visible:ring-3 focus-visible:ring-ring/30"
      onclick={() => openUrl("https://db-ip.com")}>DB-IP</button
    >, used under CC BY 4.0.
  </p>
</div>
