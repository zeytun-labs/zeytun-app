<script lang="ts">
  import { Button } from "$lib/components/ui/button/index.js";
  import { Input } from "$lib/components/ui/input/index.js";
  import { Label } from "$lib/components/ui/label/index.js";
  import { HugeiconsIcon } from "@hugeicons/svelte";
  import { RefreshIcon } from "@hugeicons/core-free-icons";
  import type { SettingsDraft } from "../settings-draft.svelte";
  import SettingsSection from "../settings-section.svelte";
  import type { ConnectivityField } from "../validation";

  let { draft }: { draft: SettingsDraft } = $props();

  const fields: { key: ConnectivityField; label: string; placeholder: string }[] = [
    {
      key: "internetTestUrl",
      label: "Internet Test URL",
      placeholder: "http://google.com/",
    },
    {
      key: "proxyTestUrl",
      label: "Proxy Test URL",
      placeholder: "http://cp.cloudflare.com/",
    },
    {
      key: "networkConfigUrl",
      label: "Network Config URL",
      placeholder: "https://mensura.cdn-apple.com/api/v1/gm/config",
    },
    {
      key: "stunServer",
      label: "STUN Server",
      placeholder: "stun.voipgate.com:3478",
    },
  ];

  function clearError(key: ConnectivityField) {
    if (draft.errors[key]) {
      draft.errors = { ...draft.errors, [key]: undefined };
    }
  }
</script>

<SettingsSection
  title="Diagnostics"
  description="Endpoints used by the network quality and STUN tests."
>
  {#snippet action()}
    <Button
      variant="outline"
      size="sm"
      onclick={() => draft.resetConnectivity()}
      class="gap-1.5"
    >
      <HugeiconsIcon icon={RefreshIcon} class="size-3" />
      Reset
    </Button>
  {/snippet}

  {#each fields as field (field.key)}
    <div class="space-y-1.5 px-5 py-4">
      <Label for={field.key}>{field.label}</Label>
      <Input
        id={field.key}
        bind:value={draft.settings[field.key]}
        placeholder={field.placeholder}
        aria-invalid={draft.errors[field.key] ? "true" : undefined}
        oninput={() => clearError(field.key)}
      />
      {#if draft.errors[field.key]}
        <p class="text-xs text-destructive">{draft.errors[field.key]}</p>
      {/if}
    </div>
  {/each}
</SettingsSection>
