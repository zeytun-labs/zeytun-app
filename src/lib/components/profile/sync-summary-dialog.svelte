<script lang="ts">
  import * as Dialog from "$lib/components/ui/dialog/index.js";
  import * as Empty from "$lib/components/ui/empty/index.js";
  import { HugeiconsIcon } from "@hugeicons/svelte";
  import { AlertCircleIcon, Link04Icon } from "@hugeicons/core-free-icons";
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

  let latchedSummary = $state<SyncSummary | null>(null);
  let latchedProfileName = $state("");

  $effect(() => {
    if (summary) latchedSummary = summary;
  });

  $effect(() => {
    if (profileName) latchedProfileName = profileName;
  });

  function formatSyncTime(ms?: number | null): string {
    if (!ms || ms <= 0) return "Unknown";
    return new Date(ms).toLocaleString(undefined, {
      month: "short",
      day: "numeric",
      hour: "numeric",
      minute: "2-digit",
    });
  }
</script>

<Dialog.Root bind:open onOpenChange={(v) => !v && onClose()}>
  <Dialog.Content class="sm:max-w-md gap-5">
    <Dialog.Header>
      <Dialog.Title class="text-base font-semibold tracking-tight">Sync Summary — {latchedProfileName}</Dialog.Title>
      <Dialog.Description class="text-xs text-muted-foreground">
        Overview of changes from the latest subscription synchronization.
      </Dialog.Description>
    </Dialog.Header>

    {#if latchedSummary}
      <div class="flex flex-col gap-4">
        <!-- Proxies Metrics (Process style divide box) -->
        <div class="flex flex-col gap-1.5">
          <span class="text-[11px] font-medium text-muted-foreground uppercase tracking-wider">
            Proxy Nodes
          </span>
          <div class="flex rounded-xl bg-muted/40 border border-border/40 divide-x divide-border/30 overflow-hidden">
            <div class="flex flex-col flex-1 p-3">
              <span class="text-[11px] text-muted-foreground">Added</span>
              <span class="text-lg font-semibold tabular-nums mt-0.5 {latchedSummary.proxies_added > 0 ? 'text-emerald-500' : 'text-foreground'}">
                {latchedSummary.proxies_added > 0 ? `+${latchedSummary.proxies_added}` : "0"}
              </span>
            </div>
            <div class="flex flex-col flex-1 p-3">
              <span class="text-[11px] text-muted-foreground">Removed</span>
              <span class="text-lg font-semibold tabular-nums mt-0.5 {latchedSummary.proxies_removed > 0 ? 'text-destructive' : 'text-foreground'}">
                {latchedSummary.proxies_removed}
              </span>
            </div>
            <div class="flex flex-col flex-1 p-3">
              <span class="text-[11px] text-muted-foreground">Kept</span>
              <span class="text-lg font-semibold tabular-nums mt-0.5 text-foreground">
                {latchedSummary.proxies_kept}
              </span>
            </div>
          </div>
        </div>

        <!-- Sync Details & Routing (Process style divide list) -->
        <div class="flex flex-col gap-1.5">
          <span class="text-[11px] font-medium text-muted-foreground uppercase tracking-wider">
            Details
          </span>
          <div class="rounded-xl bg-muted/40 border border-border/40 divide-y divide-border/30 overflow-hidden text-xs">
            <div class="flex items-center justify-between px-3.5 py-2.5">
              <span class="text-muted-foreground">Orphaned Rules</span>
              <span class="font-medium tabular-nums {latchedSummary.rules_orphaned > 0 ? 'text-amber-500' : 'text-foreground'}">
                {latchedSummary.rules_orphaned}
              </span>
            </div>
            <div class="flex items-center justify-between px-3.5 py-2.5">
              <span class="text-muted-foreground">Last Sync Time</span>
              <span class="text-muted-foreground font-medium">
                {formatSyncTime(latchedSummary.at_unix_ms)}
              </span>
            </div>
          </div>
        </div>

        <!-- Orphaned Rules Warning -->
        {#if latchedSummary.rules_orphaned > 0}
          <div class="rounded-xl border border-amber-500/25 bg-amber-500/10 p-3 text-xs text-amber-500 flex items-start gap-2.5">
            <HugeiconsIcon icon={AlertCircleIcon} class="size-4 shrink-0 mt-0.5" />
            <span class="leading-relaxed">
              {latchedSummary.rules_orphaned} rule(s) targeted removed proxies and were redirected to Direct. Check the Rules page to reassign them.
            </span>
          </div>
        {/if}

        <!-- Errors List -->
        {#if latchedSummary.errors && latchedSummary.errors.length > 0}
          <div class="rounded-xl border border-destructive/25 bg-destructive/10 p-3 text-xs text-destructive flex flex-col gap-1.5">
            <div class="flex items-center gap-1.5 font-medium">
              <HugeiconsIcon icon={AlertCircleIcon} class="size-4 shrink-0" />
              <span>Errors encountered during sync ({latchedSummary.errors.length})</span>
            </div>
            <div class="space-y-1 pl-5">
              {#each latchedSummary.errors as err}
                <p class="text-[11px] opacity-90 break-all">{err}</p>
              {/each}
            </div>
          </div>
        {/if}
      </div>
    {:else}
      <Empty.Root class="py-8 border border-dashed border-border/40 rounded-xl bg-muted/20">
        <Empty.Media variant="icon">
          <HugeiconsIcon icon={Link04Icon} class="size-5 text-muted-foreground" />
        </Empty.Media>
        <Empty.Header>
          <Empty.Title class="text-xs font-semibold">No sync record</Empty.Title>
          <Empty.Description class="text-[11px]">
            This subscription hasn't recorded any sync metrics yet.
          </Empty.Description>
        </Empty.Header>
      </Empty.Root>
    {/if}
  </Dialog.Content>
</Dialog.Root>
