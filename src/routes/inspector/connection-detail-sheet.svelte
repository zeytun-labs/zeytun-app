<script lang="ts">
  import * as Sheet from "$lib/components/ui/sheet";
  import { Button } from "$lib/components/ui/button";
  import { Separator } from "$lib/components/ui/separator";
  import { formatBytes } from "$lib/utils";
  import ProcessIcon from "../process/components/process-icon.svelte";
  import type { ConnDto } from "$lib/core/types";
  import { fmtTime, formatDuration, methodOf, statusColor } from "./lib";

  interface Props {
    connection: ConnDto | null;
    policyName: (tag: string) => string;
    onClose: () => void;
    onAddHostRule: (r: ConnDto) => void;
    onAddProcessRule: (r: ConnDto) => void;
  }

  let {
    connection,
    policyName,
    onClose,
    onAddHostRule,
    onAddProcessRule,
  }: Props = $props();
</script>

<Sheet.Root
  open={connection !== null}
  onOpenChange={(o) => {
    if (!o) onClose();
  }}
>
  <Sheet.Content side="right" class="flex w-105 flex-col sm:max-w-105">
    {#if connection}
      {@const r = connection}
      <Sheet.Header>
        <Sheet.Title class="flex items-center gap-2">
          <ProcessIcon name={r.processName} path={r.processPath} class="size-6" />
          <span class="truncate">{r.processName || "System"}</span>
        </Sheet.Title>
      </Sheet.Header>

      <div class="flex-1 space-y-3 overflow-y-auto text-sm px-4">
        <div class="flex items-center gap-2">
          <span class="size-2 rounded-full {statusColor(r.status)}"></span>
          <span class="capitalize">{r.status}</span>
        </div>

        <div class="rounded-xl bg-muted border divide-y divide-border">
          {#each [["Host", r.host], ["Address", r.address], ["Network", (r.network || "—").toUpperCase()], ["Policy", policyName(r.policy)], ["Start", fmtTime(r.createdAt)], ["End", r.endedAt ? fmtTime(r.endedAt) : "—"], ["Duration", formatDuration(r)]] as [label, value] (label)}
            <div class="flex justify-between gap-3 px-4 py-2">
              <span class="text-muted-foreground shrink-0">{label}</span>
              <span class="truncate text-right" title={String(value)}
                >{value}</span
              >
            </div>
          {/each}
        </div>

        <!-- Process Path — full executable path, selectable for copy -->
        {#if r.processPath}
          <div class="rounded-xl bg-muted border px-4 py-2 flex flex-col gap-1">
            <span class="text-muted-foreground text-xs">Process Path</span>
            <code
              class="select-all cursor-text break-all text-xs font-mono"
              title={r.processPath}
            >{r.processPath}</code>
          </div>
        {/if}

        <div class="flex rounded-xl bg-muted border divide-x divide-border">
          <div class="flex justify-between flex-1 px-4 py-2">
            <span class="text-muted-foreground">Upload</span>
            <span>{formatBytes(r.upBytes)}</span>
          </div>
          <div class="flex justify-between flex-1 px-4 py-2">
            <span class="text-muted-foreground">Download</span>
            <span>{formatBytes(r.downBytes)}</span>
          </div>
        </div>
      </div>

      <Sheet.Footer class="flex-col gap-2">
        <Button
          variant="outline"
          class="w-full min-w-0"
          onclick={() => onAddHostRule(r)}
          title={r.host || r.address}
        >
          <span class="truncate">Add rule for {r.host || r.address}</span>
        </Button>
        <Button
          variant="outline"
          class="w-full"
          onclick={() => onAddProcessRule(r)}
        >
          Add rule for {r.processName || "process"}
        </Button>
      </Sheet.Footer>
    {/if}
  </Sheet.Content>
</Sheet.Root>
