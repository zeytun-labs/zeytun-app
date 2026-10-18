<script lang="ts">
  import type { Snippet } from "svelte";
  let { open, children }: { open: boolean; children: Snippet } = $props();
</script>

<div
  data-selection-region
  aria-hidden={!open}
  inert={!open}
  class="-mt-3 grid shrink-0 transition-[grid-template-rows,opacity] duration-200 ease-out motion-reduce:transition-none {open ? 'grid-rows-[1fr] opacity-100' : 'grid-rows-[0fr] opacity-0'}"
>
  <div class="min-h-0 overflow-hidden">
    {@render children()}
  </div>
</div>
