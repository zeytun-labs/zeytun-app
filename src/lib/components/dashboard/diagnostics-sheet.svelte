<script lang="ts">
  import * as Sheet from "$lib/components/ui/sheet";
  import { Button } from "$lib/components/ui/button";
  import { diagnosticsStore } from "$lib/stores/diagnostics.svelte";
  import { HugeiconsIcon } from "@hugeicons/svelte";
  import { Copy01Icon } from "@hugeicons/core-free-icons";

  let { open = $bindable(false) } = $props();

  const report = $derived(diagnosticsStore.result?.report || "");
  const running = $derived(diagnosticsStore.running);

  async function copyToClipboard() {
    if (!report) return;
    try {
      await navigator.clipboard.writeText(report);
    } catch (e) {
      console.error("Failed to copy:", e);
    }
  }
</script>

<Sheet.Root bind:open>
  <Sheet.Content class="sm:max-w-2xl w-full flex flex-col gap-0 p-0">
    <Sheet.Header class="gap-0">
      <div class="flex items-center justify-between">
        <Sheet.Title>Report</Sheet.Title>
      </div>
      <Sheet.Description>
        Detailed connectivity and routing metrics.
      </Sheet.Description>
    </Sheet.Header>

    <div class="flex-1 overflow-auto">
      {#if running && !report}
        <div class="flex flex-col gap-2">
          <div class="h-4 bg-muted rounded w-1/3 animate-pulse"></div>
          <div class="h-4 bg-muted rounded w-1/2 animate-pulse"></div>
          <div class="h-4 bg-muted rounded w-2/5 animate-pulse mt-4"></div>
        </div>
      {:else if diagnosticsStore.error}
        <div class="text-sm text-red-500 bg-red-500/10 p-4 rounded-md">
          <span class="font-bold">Error:</span>
          {diagnosticsStore.error}
        </div>
      {:else if report}
        <div>
          <pre
            class="text-xs font-mono whitespace-pre-wrap wrap-break-word text-foreground p-4 pt-0 rounded-lg selection:bg-primary/30">{report}</pre>
        </div>
      {:else}
        <div
          class="text-muted-foreground text-sm flex h-full items-center justify-center"
        >
          Run diagnostics to view the report.
        </div>
      {/if}
    </div>
  </Sheet.Content>
</Sheet.Root>
