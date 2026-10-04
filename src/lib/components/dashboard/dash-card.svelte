<script lang="ts">
  import { cn } from "$lib/utils";
  import type { Snippet } from "svelte";
  import type { HTMLAttributes } from "svelte/elements";

  let {
    class: className,
    children,
    ...rest
  }: HTMLAttributes<HTMLDivElement> & {
    children?: Snippet;
  } = $props();
</script>

<div
  class={cn(
    "flex h-full flex-col overflow-hidden rounded-4xl border border-border/40",
    "bg-card/70 shadow-sm",
    className,
  )}
  {...rest}
>
  {@render children?.()}
</div>
