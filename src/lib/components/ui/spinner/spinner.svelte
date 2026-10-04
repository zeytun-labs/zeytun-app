<script lang="ts">
  import { cn } from "$lib/utils.js";
  import type { HTMLAttributes } from "svelte/elements";

  let {
    class: className,
    role = "status",
    color = "primary",
    "aria-label": ariaLabel = "Loading",
    ...restProps
  }: HTMLAttributes<HTMLDivElement> = $props();
</script>

<div
  {role}
  aria-label={ariaLabel}
  class={cn(
    "size-4 animate-spin rounded-full border-2 border-primary/30 border-t-primary",
    className,
  )}
  {...restProps}
></div>
