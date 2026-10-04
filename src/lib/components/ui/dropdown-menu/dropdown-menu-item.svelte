<script lang="ts">
	import { cn } from "$lib/utils.js";
	import { DropdownMenu as DropdownMenuPrimitive } from "bits-ui";

	let {
		ref = $bindable(null),
		class: className,
		inset,
		variant = "default",
		...restProps
	}: DropdownMenuPrimitive.ItemProps & {
		inset?: boolean;
		variant?: "default" | "destructive";
	} = $props();
</script>

<DropdownMenuPrimitive.Item
	bind:ref
	data-slot="dropdown-menu-item"
	data-inset={inset}
	data-variant={variant}
	class={cn(
		"relative z-10 flex cursor-default items-center gap-2.5 rounded-2xl px-3 py-2 text-sm font-medium outline-hidden select-none",
		"data-[variant=destructive]:text-destructive data-[variant=destructive]:*:[svg]:text-destructive",
		"not-data-[variant=destructive]:focus:text-accent-foreground not-data-[variant=destructive]:data-highlighted:text-accent-foreground",
		"data-inset:pl-9.5 data-[inset]:pl-8 data-disabled:pointer-events-none data-disabled:opacity-50",
		"[&_svg:not([class*='size-'])]:size-4 [&_svg]:pointer-events-none [&_svg]:shrink-0",
		"group/dropdown-menu-item",
		className
	)}
	{...restProps}
/>
