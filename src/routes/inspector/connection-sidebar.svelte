<script lang="ts">
  import * as Tabs from "$lib/components/ui/tabs";
  import { ScrollArea } from "$lib/components/ui/scroll-area";
  import { Separator } from "$lib/components/ui/separator";
  import ProcessIcon from "../process/components/process-icon.svelte";
  import { HugeiconsIcon } from "@hugeicons/svelte";
  import { Cancel01Icon } from "@hugeicons/core-free-icons";
  import { slide } from "svelte/transition";
  import type { ConnDto } from "$lib/core/types";
  import type { GroupBy } from "./lib";

  interface Props {
    rows: ConnDto[];
    groupBy: GroupBy;
    selectedGroup: string | null;
    onGroupByChange: (g: GroupBy) => void;
    onSelectGroup: (g: string | null) => void;
  }

  let { rows, groupBy, selectedGroup, onGroupByChange, onSelectGroup }: Props =
    $props();

  // Keep the chip mounted through its exit transition (otherwise it vanishes
  // instantly and the shrink-out never plays), and remember the last selected
  // name so the chip doesn't go blank mid-transition.
  let chipGone = $state(false);
  let lastGroup = $state<string | null>(null);
  $effect(() => {
    if (selectedGroup !== null) lastGroup = selectedGroup;
    chipGone = selectedGroup === null;
  });
  const chipLabel = $derived(selectedGroup ?? lastGroup);

  function keyOf(r: ConnDto): string {
    return groupBy === "client" ? r.processName : r.host;
  }

  // Unique groups with a count and a representative process path (client icon).
  const groups = $derived.by(() => {
    const map = new Map<string, { count: number; path: string }>();
    for (const r of rows) {
      const k = keyOf(r) || "—";
      const e = map.get(k) ?? { count: 0, path: r.processPath };
      e.count += 1;
      if (!e.path) e.path = r.processPath;
      map.set(k, e);
    }
    return [...map.entries()].sort((a, b) => b[1].count - a[1].count);
  });
</script>

<div class="flex h-full min-h-0 flex-col">
  <div class="border-b border-border/40 p-3">
    <Tabs.Root
      value={groupBy}
      onValueChange={(v) => onGroupByChange(v as GroupBy)}
    >
      <Tabs.List class="w-full" variant="primary">
        <Tabs.Trigger value="client" class="flex-1">By Client</Tabs.Trigger>
        <Tabs.Trigger value="host" class="flex-1">By Host</Tabs.Trigger>
      </Tabs.List>
    </Tabs.Root>

    {#if !chipGone}
      <div class="mt-2" transition:slide={{ duration: 200 }}>
        <button
          class="flex w-full items-center gap-1.5 rounded-full bg-primary/10 px-2.5 py-1 text-xs font-medium text-primary transition-colors hover:bg-primary/15"
          onclick={() => onSelectGroup(null)}
          title="Clear filter"
        >
          <span class="min-w-0 flex-1 truncate text-left">{chipLabel}</span>
          <HugeiconsIcon icon={Cancel01Icon} class="size-3.5 shrink-0" />
        </button>
      </div>
    {/if}
  </div>
  <ScrollArea class="min-h-0 flex-1 px-1">
    <div class="p-1.5 flex flex-col gap-0.5">
      {#each groups as [name, info] (name)}
        <button
          class="flex w-full items-center gap-2 rounded-full px-2 py-1.5 text-xs {selectedGroup ===
          name
            ? 'bg-accent text-accent-foreground'
            : 'hover:bg-accent/50'}"
          onclick={() => onSelectGroup(selectedGroup === name ? null : name)}
        >
          {#if groupBy === "client"}
            <ProcessIcon {name} path={info.path} class="size-6 shrink-0" />
            <span class="min-w-0 flex-1 truncate text-left" title={name}
              >{name}</span
            >
          {:else}
            <span
              class="min-w-0 flex-1 truncate text-left font-mono"
              title={name}>{name}</span
            >
          {/if}
          <span class="text-muted-foreground shrink-0">{info.count}</span>
        </button>
      {/each}
    </div>
  </ScrollArea>
</div>
