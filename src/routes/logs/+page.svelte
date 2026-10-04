<script lang="ts">
  import { tick } from "svelte";
  import { logsStore } from "$lib/stores/logs.svelte";
  import { Button } from "$lib/components/ui/button";
  import { HugeiconsIcon } from "@hugeicons/svelte";
  import { Delete01Icon } from "@hugeicons/core-free-icons";
  import ScrollArea from "$lib/components/ui/scroll-area/scroll-area.svelte";

  let viewportRef = $state<HTMLElement | null>(null);
  let autoScroll = $state(true);
  let skipNextScroll = false;

  $effect(() => {
    if (!viewportRef) return;
    const el = viewportRef;
    function onScroll() {
      if (skipNextScroll) {
        skipNextScroll = false;
        return;
      }
      const isAtBottom =
        Math.abs(el.scrollHeight - el.clientHeight - el.scrollTop) < 30;
      autoScroll = isAtBottom;
    }
    el.addEventListener("scroll", onScroll);
    return () => el.removeEventListener("scroll", onScroll);
  });

  // Pin to bottom when new logs arrive
  $effect(() => {
    const _ = logsStore.logs.length;
    if (autoScroll && viewportRef) {
      tick().then(() => {
        if (viewportRef) {
          skipNextScroll = true;
          viewportRef.scrollTop = viewportRef.scrollHeight;
        }
      });
    }
  });

  function levelLabel(level: number) {
    switch (level) {
      case 0: return "PANIC";
      case 1: return "FATAL";
      case 2: return "ERROR";
      case 3: return "WARN";
      case 4: return "INFO";
      case 5: return "DEBUG";
      case 6: return "TRACE";
      default: return "UNKNOWN";
    }
  }

  function levelColor(level: number) {
    switch (level) {
      case 0:
      case 1:
      case 2: return "text-red-500";
      case 3: return "text-amber-500";
      case 5: return "text-muted-foreground";
      case 6: return "text-muted-foreground/50";
      case 4:
      default: return "text-emerald-500";
    }
  }
</script>

<div class="flex flex-col h-full gap-4">
  <ScrollArea
    bind:viewportRef
    class="h-[calc(100vh-6.5rem)] flex flex-1 bg-olive-100 dark:bg-card/30 border border-border/40 rounded-4xl flex-col"
  >
    {#if logsStore.logs.length === 0}
      <div
        class="absolute inset-0 flex items-center justify-center text-muted-foreground text-sm font-mono z-10 pointer-events-none"
      >
        No logs yet...
      </div>
    {/if}

    <div
      class="p-4 text-[11px] leading-relaxed font-mono whitespace-pre-wrap break-all flex flex-col gap-1"
    >
      {#each logsStore.logs as log (log.id)}
        <div class="flex gap-3 py-px px-1">
          <span class="text-olive-500 shrink-0 select-none">[{log.time}]</span>
          <span
            class="{levelColor(log.level)} shrink-0 select-none font-medium w-12"
            >[{levelLabel(log.level)}]</span
          >
          <span class="text-foreground dark:text-olive-300 flex-1"
            >{log.message}</span
          >
        </div>
      {/each}
    </div>
  </ScrollArea>
</div>
