<script lang="ts">
  import * as DropdownMenu from "$lib/components/ui/dropdown-menu/index.js";
  import { HugeiconsIcon } from "@hugeicons/svelte";
  import {
    MoreVerticalCircle01Icon,
    Configuration01Icon,
    Cancel01Icon,
  } from "@hugeicons/core-free-icons";
  import type { ProcessStat } from "$lib/stores/dashboardStore.svelte";
  import { formatBytes, formatSpeed } from "$lib/utils";
  import { dashboardStore } from "$lib/stores/dashboardStore.svelte";
  import ProcessIcon from "./process-icon.svelte";

  let {
    selectedProcessName = $bindable(null),
    onAddRule,
    onKillConnections,
  } = $props<{
    selectedProcessName: string | null;
    onAddRule: (processName: string, processPath: string) => void;
    onKillConnections: (process: ProcessStat) => void;
  }>();

  import { Skeleton } from "$lib/components/ui/skeleton/index.js";
  import ScrollArea from "$lib/components/ui/scroll-area/scroll-area.svelte";
</script>

<ScrollArea
  class="h-[calc(100vh-9.5rem)] **:data-[slot=scroll-area-scrollbar]:hidden"
>
  <div class="flex min-h-0 min-w-0 flex-1 flex-col gap-2">
    {#if !dashboardStore.initialized}
      <div class="flex flex-col gap-2 w-full animate-in fade-in duration-500">
        {#each Array(5) as _}
          <div
            class="flex gap-4 px-4 py-3 rounded-xl items-center border border-transparent"
          >
            <Skeleton class="size-10 rounded-xl" />
            <div class="flex flex-col gap-2">
              <Skeleton class="h-4 w-32" />
            </div>
            <div class="ms-auto flex flex-col gap-2 items-end">
              <Skeleton class="h-4 w-24" />
              <Skeleton class="h-3 w-32" />
            </div>
          </div>
        {/each}
      </div>
    {:else if dashboardStore.filteredProcesses.length === 0}
      <div class="text-muted-foreground flex items-center justify-center h-20">
        No active processes found.
      </div>
    {:else}
      {#each dashboardStore.filteredProcesses as process (process.name)}
        <button
          class="bg-card/70 hover:bg-card transition-colors flex min-w-0 w-full gap-4 px-4 rounded-3xl items-center border border-border/40 hover:border-border data-[selected=true]:border-primary/30 data-[selected=true]:bg-primary/10"
          data-selected={selectedProcessName === process.name}
          onclick={() => (selectedProcessName = process.name)}
        >
          <div class="font-medium flex min-w-0 items-center gap-2 py-2">
            <div class="size-10 flex shrink-0 items-center justify-center">
              <ProcessIcon
                name={process.name}
                path={process.path}
                class="size-8"
              />
            </div>
            <div class="flex min-w-0 text-left gap-2 items-center">
              <span class="truncate">{process.name}</span>
              {#if process.subProcesses.length > 0}
                <span class="block size-1 bg-muted-foreground rounded-full"
                ></span>

                <div class="text-xs text-muted-foreground transition-colors">
                  {process.subProcesses.length} sub-processes
                </div>
              {/if}
            </div>
          </div>
          <div class="flex flex-row-reverse ms-auto gap-2 items-center">
            <span class="text-muted-foreground text-xs"
              >Total: {formatBytes(
                process.totalDownloadSinceLaunch +
                  process.totalUploadSinceLaunch,
              ).toString()}</span
            >

            <span class="block size-1 bg-muted-foreground rounded-full"></span>
            <span class="font-medium">
              {formatSpeed(process.downloadSpeed + process.uploadSpeed)}</span
            >
          </div>

          <div class="text-right">
            <DropdownMenu.Root>
              <DropdownMenu.Trigger
                class="inline-flex size-7 items-center justify-center rounded-md hover:bg-primary/20 hover:text-sidebar-primary focus-visible:outline-none"
              >
                <HugeiconsIcon icon={MoreVerticalCircle01Icon} class="size-4" />
              </DropdownMenu.Trigger>
              <DropdownMenu.Content align="end" class="w-fit">
                <DropdownMenu.Item
                  onclick={() => onAddRule(process.name, process.path)}
                >
                  <HugeiconsIcon icon={Configuration01Icon} />
                  Add Rule for {process.name}
                </DropdownMenu.Item>
                <DropdownMenu.Separator />
                <DropdownMenu.Item onclick={() => onKillConnections(process)}>
                  <HugeiconsIcon icon={Cancel01Icon} />
                  Kill Connection
                </DropdownMenu.Item>
              </DropdownMenu.Content>
            </DropdownMenu.Root>
          </div>
        </button>
      {/each}
    {/if}
  </div>
</ScrollArea>
