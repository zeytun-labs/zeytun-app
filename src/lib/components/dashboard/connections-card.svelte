<script lang="ts">
  import * as Card from "$lib/components/ui/card/index.js";
  import { cn } from "$lib/utils";
  import DashCard from "./dash-card.svelte";

  interface Props {
    headline: string;
    process: number;
  }

  let { headline, process }: Props = $props();
</script>

<DashCard class="p-5 gap-4">
  <Card.Header>
    <Card.Title
      class="flex items-center gap-2 text-muted-foreground text-xs font-medium tracking-wide"
    >
      ACTIVE CONNECTION
      <div class="ms-auto me-2 size-2 relative">
        {#if process > 0}
          <div
            class="size-2.5 bg-primary rounded-full absolute"
            aria-hidden="true"
          ></div>
          <div
            class="size-2.5 bg-primary rounded-full animate-ping"
            aria-hidden="true"
          ></div>
        {:else}
          <div
            class="size-2.5 bg-red-500 rounded-full absolute"
            aria-hidden="true"
          ></div>
        {/if}
      </div>
    </Card.Title>
  </Card.Header>
  <Card.Content class="flex flex-col h-full justify-between gap-2">
    <div class="flex items-center gap-2">
      <span class="text-4xl font-semibold tracking-tight leading-none">{headline}</span>
    </div>

    <div
      class={cn(
        "flex min-w-0 flex-row items-center justify-between gap-0.5 rounded-xl border border-border/30",
        "bg-foreground/5 px-3 py-2",
      )}
    >
      <div class="text-muted-foreground text-[10px] font-medium tracking-wider uppercase">Process</div>
      <div class="flex items-end gap-1 text-sm font-medium">{process}</div>
    </div>
  </Card.Content>
</DashCard>
