<script lang="ts">
  import * as Dialog from "$lib/components/ui/dialog/index.js";
  import { Button } from "$lib/components/ui/button/index.js";
  import { Label } from "$lib/components/ui/label/index.js";
  import * as Select from "$lib/components/ui/select/index.js";
  import { coreRetryRulesets } from "$lib/core/api";
  import { notificationStore } from "$lib/stores/notifications.svelte";
  import { profileStore } from "$lib/stores/profile.svelte";
  import { toast } from "svelte-sonner";
  import { errorMessage } from "$lib/errors";

  type OutboundOption = { tag: string; name: string };
  type OutboundGroup = { label?: string; options: OutboundOption[] };

  interface Props {
    open: boolean;
    actionGroups: OutboundGroup[];
    onSuccess?: () => void;
  }

  let { open = $bindable(false), actionGroups, onSuccess }: Props = $props();

  let outboundTag = $state("");
  let saving = $state(false);

  const flat = $derived(actionGroups.flatMap((g) => g.options));

  $effect(() => {
    if (open) {
      outboundTag =
        flat.find((o) => o.tag === "direct")?.tag ||
        flat[0]?.tag ||
        "";
    }
  });

  function labelFor(tag: string) {
    return flat.find((o) => o.tag === tag)?.name || tag;
  }

  async function submit() {
    if (!outboundTag) return;
    saving = true;
    try {
      const remoteTags = (profileStore.profile?.rule_sets ?? [])
        .filter((r) => r.type === "remote")
        .map((r) => r.tag);
      notificationStore.markRemoteDownloading(remoteTags);
      await coreRetryRulesets(outboundTag);
      // toast.success("Retrying ruleset downloads", {
      //   description: `All remote via ${labelFor(outboundTag)}`,
      // });
      open = false;
      onSuccess?.();
    } catch (e) {
      toast.error(errorMessage(e));
    } finally {
      saving = false;
    }
  }
</script>

<Dialog.Root bind:open>
  <Dialog.Content class="sm:max-w-md">
    <Dialog.Header>
      <Dialog.Title>Retry ruleset download</Dialog.Title>
      <Dialog.Description>
        Pick an outbound for the download path (http_client detour). Applied to
        <strong>all remote rulesets</strong>. Core restarts and re-fetches.
      </Dialog.Description>
    </Dialog.Header>

    <div class="space-y-4 py-2">
      <div class="space-y-2">
        <Label>Outbound</Label>
        <Select.Root type="single" bind:value={outboundTag}>
          <Select.Trigger class="w-full">
            {labelFor(outboundTag) || "Select…"}
          </Select.Trigger>
          <Select.Content>
            {#each actionGroups as group, i (group.label ?? i)}
              {#if group.options.length > 0}
                <Select.Group>
                  {#if group.label}
                    <Select.Label>{group.label}</Select.Label>
                  {/if}
                  {#each group.options as o (o.tag)}
                    <Select.Item value={o.tag} label={o.name}>
                      {o.name}
                    </Select.Item>
                  {/each}
                </Select.Group>
              {/if}
            {/each}
          </Select.Content>
        </Select.Root>
      </div>
    </div>

    <Dialog.Footer>
      <Button variant="outline" onclick={() => (open = false)}>Cancel</Button>
      <Button disabled={!outboundTag || saving} onclick={submit}>
        {saving ? "Retrying…" : "Retry all remote"}
      </Button>
    </Dialog.Footer>
  </Dialog.Content>
</Dialog.Root>
