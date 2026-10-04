<script lang="ts">
  import { Switch } from "$lib/components/ui/switch/index.js";
  import { Button } from "$lib/components/ui/button/index.js";
  import * as Tooltip from "$lib/components/ui/tooltip/index.js";
  import type { SettingsDraft } from "../settings-draft.svelte";
  import SettingRow from "../setting-row.svelte";
  import SettingsSection from "../settings-section.svelte";
  import Spinner from "$lib/components/ui/spinner/spinner.svelte";
  import { flushDnsCache } from "$lib/core/api";
  import { toast } from "svelte-sonner";
  import { HugeiconsIcon } from "@hugeicons/svelte";
  import { Delete01Icon } from "@hugeicons/core-free-icons";
  import { profileStore } from "$lib/stores/profile.svelte";
  import { errorMessage } from "$lib/errors";

  let { draft }: { draft: SettingsDraft } = $props();

  let isFlushing = $state(false);

  async function flushCache() {
    isFlushing = true;
    try {
      const result = await flushDnsCache();
      toast.success(
        result || "Core DNS and FakeIP caches flushed successfully.",
      );
    } catch (error) {
      console.error("Failed to flush core DNS and FakeIP caches:", error);
      const details = errorMessage(error);
      const msg = profileStore.isRunning
        ? `Could not flush the core DNS and FakeIP caches: ${details}`
        : "Could not flush the core DNS and FakeIP caches because the core is not running.";
      toast.error(msg);
    } finally {
      isFlushing = false;
    }
  }
</script>

<SettingsSection
  title="DNS"
  description="FakeIP engine and cache. Servers and DNS rules live on the DNS page."
>
  <SettingRow
    label="FakeIP Engine"
    description="Reroutes domain resolution to internal pool addresses for proxy routing. Speeds up DNS responses and prevents local DNS leaks."
  >
    <Switch bind:checked={draft.settings.enableFakeIp} />
  </SettingRow>

  <SettingRow
    label="DNS Cache"
    description="Clear the core's DNS response cache and FakeIP mappings."
  >
    <Tooltip.Provider>
      <Tooltip.Root>
        <Tooltip.Trigger>
          {#snippet child({ props })}
            <!-- A disabled button emits no pointer events, so the reason has to
                 hang off a wrapper the tooltip can see. -->
            <span {...props}>
              <Button
                variant="outline"
                size="sm"
                class="gap-1.5"
                onclick={flushCache}
                disabled={isFlushing || !profileStore.isRunning}
              >
                {#if isFlushing}
                  <Spinner class="size-3.5" />
                {:else}
                  <HugeiconsIcon icon={Delete01Icon} class="size-3.5" />
                {/if}
                {isFlushing ? "Flushing…" : "Flush Cache"}
              </Button>
            </span>
          {/snippet}
        </Tooltip.Trigger>
        {#if !profileStore.isRunning}
          <Tooltip.Content side="left">
            <p>The core is not running.</p>
          </Tooltip.Content>
        {/if}
      </Tooltip.Root>
    </Tooltip.Provider>
  </SettingRow>
</SettingsSection>
