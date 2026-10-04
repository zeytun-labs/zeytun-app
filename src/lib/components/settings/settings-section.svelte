<script lang="ts">
  import type { Snippet } from "svelte";

  // One header + card shell for every settings page. The chrome string used to
  // be copy-pasted into each section, which is how the headers drifted apart.
  let {
    title,
    description,
    action,
    children,
  }: {
    title: string;
    description?: string;
    /** Optional control rendered at the header's trailing edge (e.g. Reset). */
    action?: Snippet;
    children: Snippet;
  } = $props();
</script>

<section class="space-y-3">
  <div class="flex items-start justify-between gap-4">
    <div class="min-w-0">
      <h3 class="text-base font-medium">{title}</h3>
      {#if description}
        <p class="mt-0.5 text-sm text-muted-foreground">{description}</p>
      {/if}
    </div>
    {#if action}
      <div class="shrink-0">{@render action()}</div>
    {/if}
  </div>
  <div
    class="overflow-hidden rounded-2xl border border-border/40 bg-card/70 divide-y divide-border/40"
  >
    {@render children()}
  </div>
</section>
