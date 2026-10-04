<script lang="ts">
  import type { Snippet } from "svelte";
  import { Label } from "$lib/components/ui/label/index.js";

  let {
    label,
    description,
    for: forAttr,
    children,
  }: {
    label: string;
    description?: string;
    for?: string;
    children: Snippet;
  } = $props();
</script>

<div class="flex items-center justify-between gap-6 px-5 py-4">
  <div class="min-w-0 space-y-0.5">
    <Label for={forAttr}>{label}</Label>
    {#if description}
      <p class="text-xs text-muted-foreground">{description}</p>
    {/if}
  </div>
  <div class="shrink-0">
    {@render children()}
  </div>
</div>
