<script lang="ts">
  import { Button } from "$lib/components/ui/button";
  import { HugeiconsIcon } from "@hugeicons/svelte";
  import {
    Configuration01Icon,
    Cancel01Icon,
  } from "@hugeicons/core-free-icons";
  import type { ProcessStat } from "$lib/stores/dashboardStore.svelte";
  import { formatBytes, formatSpeed } from "$lib/utils";
  import ProcessIcon from "./process-icon.svelte";

  // ponytail: SubProcessStat import not needed — accessed via ProcessStat.subProcesses

  interface Props {
    process: ProcessStat;
    onClose: () => void;
  }

  let { process, onClose }: Props = $props();
</script>

<div
  class="flex h-full min-h-0 w-full flex-col overflow-hidden rounded-4xl border border-border/40 bg-card/70"
>
  <div class="flex items-center gap-3 border-b border-border/40 px-4 py-3">
    <div class="size-8 flex items-center justify-center overflow-hidden shrink-0">
      <ProcessIcon name={process.name} path={process.path}  />
    </div>
    <div class="flex flex-col min-w-0 flex-1">
      <span class="truncate leading-tight font-medium">{process.name}</span>
    </div>
    <Button
      variant="ghost"
      size="icon"
      class="text-muted-foreground shrink-0 size-7"
      onclick={onClose}
    >
      <HugeiconsIcon icon={Cancel01Icon} class="size-4" />
    </Button>
  </div>

  <div class="flex-1 space-y-4 overflow-y-auto overflow-x-hidden px-4 py-4 text-sm min-h-0">
    <div class="flex flex-col gap-2">
      <h4 class="text-xs text-muted-foreground uppercase tracking-wider">
        Bandwidth
      </h4>
      <div class="flex rounded-xl bg-muted border divide-x divide-border">
        <div class="flex justify-between flex-1 min-w-0 px-4 py-2">
          <span class="text-muted-foreground">Download</span>
          <span class="truncate">{formatSpeed(process.downloadSpeed)}</span>
        </div>
        <div class="flex justify-between flex-1 min-w-0 px-4 py-2">
          <span class="text-muted-foreground">Upload</span>
          <span class="truncate">{formatSpeed(process.uploadSpeed)}</span>
        </div>
      </div>
    </div>

    <div class="flex flex-col gap-2">
      <h4 class="text-xs text-muted-foreground uppercase tracking-wider">
        Connections
      </h4>
      <div class="flex rounded-xl bg-muted border divide-x divide-border">
        <div class="flex justify-between flex-1 min-w-0 px-4 py-2">
          <span class="text-muted-foreground">Active</span>
          <span>{process.activeConnections}</span>
        </div>
        <div class="flex justify-between flex-1 min-w-0 px-4 py-2">
          <span class="text-muted-foreground">Closed</span>
          <span>{process.closedConnections}</span>
        </div>
      </div>
    </div>

    <div class="flex flex-col gap-2">
      <h4 class="text-xs text-muted-foreground uppercase tracking-wider">
        Details
      </h4>
      <div class="bg-muted border rounded-xl divide-y divide-border min-w-0">
        <div class="flex flex-col px-4 py-2 gap-1.5 min-w-0">
          <span class="text-muted-foreground">Top Host</span>
          <span class="font-mono truncate" title={process.topHost}>
            {process.topHost}
          </span>
        </div>
        <div class="flex flex-col px-4 py-2 gap-1.5 min-w-0">
          <span class="text-muted-foreground">Process Path</span>
          <code
            class="select-all cursor-text break-all text-xs font-mono"
            title={process.path}
          >{process.path || "System"}</code>
        </div>
      </div>
    </div>

    {#if process.subProcesses && process.subProcesses.length > 0}
      <div class="flex flex-col gap-2 min-w-0">
        <h4 class="text-xs text-muted-foreground uppercase tracking-wider">
          Sub-Processes ({process.subProcesses.length})
        </h4>
        <div class="bg-muted border rounded-xl divide-y divide-border min-w-0">
          {#each process.subProcesses as sub}
            <div class="flex flex-col px-4 py-2 gap-1 min-w-0">
              <div class="flex justify-between items-center gap-2 min-w-0">
                <span class="text-xs font-medium truncate flex-1" title={sub.name}>{sub.name}</span>
                <span class="text-xs shrink-0">{formatSpeed(sub.downloadSpeed + sub.uploadSpeed)}</span>
              </div>
              <span class="text-[10px] text-muted-foreground font-mono truncate" title={sub.path}>{sub.path}</span>
              <div class="flex gap-3 text-[10px] text-muted-foreground">
                <span>{sub.activeConnections} active</span>
                <span>{sub.closedConnections} closed</span>
              </div>
            </div>
          {/each}
        </div>
      </div>
    {/if}

    <div class="flex flex-col gap-2">
      <h4 class="text-xs text-muted-foreground uppercase tracking-wider">
        Traffic
      </h4>
      <div class="bg-muted border rounded-xl divide-y divide-border">
        <div class="flex justify-between items-center gap-1 px-4 py-2">
          <span class="text-muted-foreground">Today</span>
          <div class="flex gap-2">
            <span>↓ {formatBytes(process.todayDownload).toString()}</span>
            <span>↑ {formatBytes(process.todayUpload).toString()}</span>
          </div>
        </div>
        <div class="flex justify-between items-center gap-1 px-4 py-2">
          <span class="text-muted-foreground">Since Launch</span>
          <div class="flex gap-2">
            <span
              >↓ {formatBytes(
                process.totalDownloadSinceLaunch,
              ).toString()}</span
            >
            <span
              >↑ {formatBytes(
                process.totalUploadSinceLaunch,
              ).toString()}</span
            >
          </div>
        </div>
      </div>
    </div>
  </div>

</div>
