<script lang="ts">
  import * as Sheet from "$lib/components/ui/sheet";
  import { Button } from "$lib/components/ui/button";
  import { Skeleton } from "$lib/components/ui/skeleton";
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
  <Sheet.Content class="sm:max-w-2xl! w-full flex flex-col gap-0 p-0">
    <Sheet.Header class="px-6 pt-6 pb-4 shrink-0 pr-12">
      <Sheet.Title>Report</Sheet.Title>
      <Sheet.Description>
        Detailed connectivity and routing metrics.
      </Sheet.Description>
    </Sheet.Header>

    <div class="flex-1 min-h-0 overflow-auto px-6 pb-6">
      {#if running && !report}
        <div class="flex flex-col gap-3 py-2">
          <Skeleton class="h-4 w-1/3" />
          <Skeleton class="h-4 w-1/2" />
          <Skeleton class="h-4 w-2/5 mt-2" />
        </div>
      {:else if diagnosticsStore.error}
        <div class="text-sm text-red-500 bg-red-500/10 p-4 rounded-md">
          <span class="font-bold">Error:</span>
          {diagnosticsStore.error}
        </div>
      {:else if report}
        <div>
          <pre
            class="text-xs font-mono whitespace-pre-wrap wrap-break-word text-foreground p-4 rounded-lg bg-muted/40 border selection:bg-primary/30">{report}</pre>
        </div>
      {:else}
        <div
          class="text-muted-foreground text-sm flex h-full items-center justify-center"
        >
          Run diagnostics to view the report.
        </div>
      {/if}
    </div>

    {#if report}
      <Sheet.Footer class="p-4 border-t border-border flex flex-row justify-end gap-2 shrink-0">
        <Button variant="outline" size="sm" onclick={copyToClipboard}>
          <HugeiconsIcon icon={Copy01Icon} class="mr-1.5 size-4" />
          Copy Report
        </Button>
      </Sheet.Footer>
    {/if}
  </Sheet.Content>
</Sheet.Root>
