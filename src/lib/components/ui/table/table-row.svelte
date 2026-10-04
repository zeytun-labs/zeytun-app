<script lang="ts">
	import { cn, type WithElementRef } from "$lib/utils.js";
	import type { HTMLAttributes } from "svelte/elements";

	let {
		ref = $bindable(null),
		class: className,
		children,
		...restProps
	}: WithElementRef<HTMLAttributes<HTMLTableRowElement>> = $props();
</script>

<tr bind:this={ref} data-slot="table-row" class={cn("hover:bg-muted/30 data-[state=selected]:bg-muted/40 transition-colors [&>td:first-child]:rounded-l-lg [&>td:last-child]:rounded-r-lg", className)} {...restProps}>
	{@render children?.()}
</tr>
