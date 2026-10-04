<script lang="ts">
  import * as Dialog from "$lib/components/ui/dialog/index.js";
  import { Button } from "$lib/components/ui/button/index.js";
  import type { SyncSummary } from "$lib/core/types";

  interface Props {
    open: boolean;
    profileName: string;
    summary: SyncSummary | null;
    onClose: () => void;
  }

  let {
    open = $bindable(false),
    profileName,
    summary,
    onClose,
  }: Props = $props();

  const rows = $derived(
    summary
      ? [
          { label: "Proxies added", value: summary.proxies_added },
          { label: "Proxies removed", value: summary.proxies_removed },
          { label: "Proxies kept", value: summary.proxies_kept },
          { label: "Rules orphaned", value: summary.rules_orphaned },
        ]
      : [],
  );
</script>

<Dialog.Root bind:open onOpenChange={(v) => !v && onClose()}>
  <Dialog.Content class="sm:max-w-md">
    <Dialog.Header>
      <Dialog.Title>Update summary — {profileName}</Dialog.Title>
      <Dialog.Description>
        Result of the latest subscription sync for this profile.
      </Dialog.Description>
    </Dialog.Header>

    {#if summary}
      <div class="grid gap-2 py-2">
        {#each rows as row (row.label)}
          <div class="flex items-center justify-between text-sm">
            <span class="text-muted-foreground">{row.label}</span>
            <span class="font-medium tabular-nums">{row.value}</span>
          </div>
        {/each}

        {#if summary.rules_orphaned > 0}
          <p
            class="text-muted-foreground mt-1 rounded-md bg-muted/40 px-3 py-2 text-xs"
          >
            {summary.rules_orphaned} rule(s) targeted a removed proxy and were repointed
            to Direct. Review them on the Rules page.
          </p>
        {/if}

        {#if summary.errors.length > 0}
          <div class="mt-1 flex flex-col gap-1">
            {#each summary.errors as err (err)}
              <p class="text-destructive text-xs">{err}</p>
            {/each}
          </div>
        {/if}
      </div>
    {:else}
      <p class="text-muted-foreground py-4 text-sm">No summary available.</p>
    {/if}

    <Dialog.Footer>
      <Button onclick={() => { open = false; onClose(); }}>Close</Button>
    </Dialog.Footer>
  </Dialog.Content>
</Dialog.Root>
