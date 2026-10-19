<script lang="ts">
  import type { Snippet } from "svelte";
  import { Button } from "$lib/components/ui/button/index.js";
  import { HugeiconsIcon } from "@hugeicons/svelte";
  import { Delete01Icon, PauseIcon, PlayIcon } from "@hugeicons/core-free-icons";

  let {
    selectedCount,
    itemLabel = "rule",
    itemLabelPlural = `${itemLabel}s`,
    showStatus = true,
    children,
    canEnable,
    canDisable,
    saving,
    onEnable,
    onDisable,
    onDelete,
  }: {
    selectedCount: number;
    itemLabel?: string;
    itemLabelPlural?: string;
    showStatus?: boolean;
    children?: Snippet;
    canEnable: boolean;
    canDisable: boolean;
    saving: boolean;
    onEnable: () => void;
    onDisable: () => void;
    onDelete: () => void;
  } = $props();
</script>

<div
  role="group"
  aria-label={`Selected ${itemLabel} actions`}
  aria-busy={saving}
  class="flex flex-wrap items-center justify-between gap-2 rounded-xl border border-border/70 bg-muted/20 px-3 py-2"
>
  <span class="text-sm text-muted-foreground tabular-nums" aria-live="polite">
    {selectedCount} {selectedCount === 1 ? itemLabel : itemLabelPlural} selected
  </span>
  <div class="flex flex-wrap items-center gap-2">
    {#if showStatus}
      <Button variant="outline" size="sm" disabled={saving || !canEnable} onclick={onEnable}>
        <HugeiconsIcon icon={PlayIcon} data-icon="inline-start" />
        Enable selected
      </Button>
      <Button variant="outline" size="sm" disabled={saving || !canDisable} onclick={onDisable}>
        <HugeiconsIcon icon={PauseIcon} data-icon="inline-start" />
        Disable selected
      </Button>
    {/if}
    {@render children?.()}
    <Button variant="destructive" size="sm" disabled={saving} onclick={onDelete}>
      <HugeiconsIcon icon={Delete01Icon} data-icon="inline-start" />
      Delete selected
    </Button>
  </div>
</div>
