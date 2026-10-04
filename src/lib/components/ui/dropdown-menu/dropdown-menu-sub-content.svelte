<script lang="ts">
	import { DropdownMenu as DropdownMenuPrimitive } from "bits-ui";
	import { cn } from "$lib/utils.js";
	import { slidingHighlight } from "$lib/actions/sliding-highlight";

	let {
		ref = $bindable(null),
		class: className,
		...restProps
	}: DropdownMenuPrimitive.SubContentProps = $props();

	$effect(() => {
		if (!ref) return;
		return slidingHighlight(ref).destroy;
	});
</script>

<DropdownMenuPrimitive.SubContent
	bind:ref
	data-slot="dropdown-menu-sub-content"
	class={cn(
		"data-open:animate-in data-closed:animate-out data-closed:fade-out-0 data-open:fade-in-0 data-closed:zoom-out-95 data-open:zoom-in-95 data-[side=bottom]:slide-in-from-top-2 data-[side=left]:slide-in-from-right-2 data-[side=right]:slide-in-from-left-2 data-[side=top]:slide-in-from-bottom-2 ring-foreground/5 dark:ring-foreground/10 text-popover-foreground min-w-36 rounded-3xl p-1.5 shadow-lg ring-1 duration-100 w-auto relative bg-popover/70 before:pointer-events-none before:absolute before:inset-0 before:-z-1 before:rounded-[inherit] before:backdrop-blur-2xl before:backdrop-saturate-150 **:data-[slot$=-item]:focus:text-accent-foreground **:data-[slot$=-item]:data-highlighted:text-accent-foreground **:data-[slot$=-separator]:bg-foreground/5 **:data-[slot$=-trigger]:focus:text-accent-foreground **:data-[slot$=-trigger]:aria-expanded:text-accent-foreground **:data-[variant=destructive]:text-destructive! **:data-[variant=destructive]:**:text-destructive!",
		className
	)}
	{...restProps}
/>
