<script lang="ts">
  import { onMount } from "svelte";
  import { getVersion } from "@tauri-apps/api/app";
  import { Button } from "$lib/components/ui/button/index.js";
  import { HugeiconsIcon } from "@hugeicons/svelte";
  import { SystemUpdate01Icon } from "@hugeicons/core-free-icons";
  import { Spinner } from "$lib/components/ui/spinner/index.js";
  import { updaterStore } from "$lib/stores/updater.svelte";
  import { settingsStore } from "$lib/stores/settings.svelte";
  import { Switch } from "$lib/components/ui/switch";
  import SettingRow from "../setting-row.svelte";
  import SettingsSection from "../settings-section.svelte";

  let version = $state("—");

  const phase = $derived(updaterStore.phase);
  const update = $derived(updaterStore.update);

  onMount(() => {
    getVersion()
      .then((v) => (version = v))
      .catch((e) => console.error("Failed to read app version:", e));
  });
</script>

<SettingsSection title="About" description="App information and updates.">
  <SettingRow
    label="Zeytun"
    description={`Version v${version}`}
  >
    {#if phase === "downloading"}
      <!-- Download runs in the store, so leaving this page does not cancel it. -->
      <div class="w-48 space-y-1.5">
        <div class="flex items-center justify-between text-xs">
          <span class="flex items-center gap-1.5 text-muted-foreground">
            <Spinner class="size-3" />
            <span>Downloading {update?.version ? `v${update.version}` : ""}</span>
          </span>
          <span class="font-mono text-xs tabular-nums text-muted-foreground">
            {Math.round(updaterStore.progress * 100)}%
          </span>
        </div>
        <div class="h-1.5 w-full overflow-hidden rounded-full bg-muted">
          <div
            class="h-full rounded-full bg-primary transition-[width] duration-200"
            style="width: {Math.round(updaterStore.progress * 100)}%"
          ></div>
        </div>
      </div>
    {:else if phase === "ready"}
      <Button
        variant="default"
        size="sm"
        class="gap-1.5"
        onclick={() => void updaterStore.install()}
      >
        <HugeiconsIcon icon={SystemUpdate01Icon} class="size-3.5" />
        {update?.version ? `Install v${update.version} & Restart` : "Install and Restart"}
      </Button>
    {:else if phase === "available"}
      <Button
        variant="default"
        size="sm"
        class="gap-1.5"
        onclick={() => void updaterStore.download()}
      >
        <HugeiconsIcon icon={SystemUpdate01Icon} class="size-3.5" />
        {update?.version ? `Download v${update.version}` : "Download Update"}
      </Button>
    {:else}
      <Button
        variant="secondary"
        size="sm"
        class="gap-1.5"
        disabled={phase === "checking"}
        onclick={() => void updaterStore.check()}
      >
        {#if phase === "checking"}
          <Spinner class="size-3.5 border-secondary-foreground/30 border-t-secondary-foreground" />
        {:else}
          <HugeiconsIcon icon={SystemUpdate01Icon} class="size-3.5" />
        {/if}
        {phase === "checking" ? "Checking…" : "Check for Updates"}
      </Button>
    {/if}
  </SettingRow>
  <SettingRow
    label="Receive pre-release updates"
    description="Include alpha and beta releases when checking for updates."
  >
    <Switch
      checked={settingsStore.settings.receivePrereleaseUpdates}
      disabled={phase === "checking" || phase === "downloading" || phase === "ready"}
      onCheckedChange={(checked) => {
        settingsStore.settings.receivePrereleaseUpdates = checked;
        settingsStore.save();
        void updaterStore.check();
      }}
      aria-label="Receive pre-release updates"
    />
  </SettingRow>
</SettingsSection>
