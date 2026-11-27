<script lang="ts">
	import { DropdownMenu as DropdownMenuPrimitive } from "bits-ui";
	import { cn, type WithoutChildrenOrChild } from "$lib/utils.js";
	import DropdownMenuPortal from "./dropdown-menu-portal.svelte";
	import type { ComponentProps } from "svelte";
	import { slidingHighlight } from "$lib/actions/sliding-highlight";

	let {
		ref = $bindable(null),
		class: className,
		align = "start",
		alignOffset = -3,
		portalProps,
		children,
		...restProps
	}: DropdownMenuPrimitive.SubContentProps & {
		portalProps?: WithoutChildrenOrChild<ComponentProps<typeof DropdownMenuPortal>>;
	} = $props();

	// The highlight lives in the scrolling viewport so it moves with the items.
	let viewportEl = $state<HTMLElement | null>(null);

	$effect(() => {
		if (!viewportEl) return;
		return slidingHighlight(viewportEl).destroy;
	});
</script>

<DropdownMenuPortal {...portalProps}>
	<DropdownMenuPrimitive.SubContent
		bind:ref
		{align}
		{alignOffset}
		data-slot="dropdown-menu-sub-content"
		class={cn(
			"data-open:animate-in data-closed:animate-out data-closed:fade-out-0 data-open:fade-in-0 data-closed:zoom-out-95 data-open:zoom-in-95 data-[side=bottom]:slide-in-from-top-2 data-[side=left]:slide-in-from-right-2 data-[side=right]:slide-in-from-left-2 data-[side=top]:slide-in-from-bottom-2 ring-foreground/5 dark:ring-foreground/10 text-popover-foreground min-w-36 rounded-3xl shadow-lg ring-1 duration-100 w-auto relative bg-popover/70 before:pointer-events-none before:absolute before:inset-0 before:-z-1 before:rounded-[inherit] before:backdrop-blur-2xl before:backdrop-saturate-150 **:data-[slot$=-item]:focus:text-accent-foreground **:data-[slot$=-item]:data-highlighted:text-accent-foreground **:data-[slot$=-separator]:bg-foreground/5 **:data-[slot$=-trigger]:focus:text-accent-foreground **:data-[slot$=-trigger]:aria-expanded:text-accent-foreground **:data-[variant=destructive]:text-destructive! **:data-[variant=destructive]:**:text-destructive! flex flex-col overflow-hidden max-h-(--bits-floating-available-height)",
			className
		)}
		{...restProps}
	>
		<div
			bind:this={viewportEl}
			data-slot="dropdown-menu-sub-content-viewport"
			class="relative min-h-0 flex-1 overflow-x-hidden overflow-y-auto overscroll-contain scroll-my-1 p-1.5"
		>
			{@render children?.()}
		</div>
	</DropdownMenuPrimitive.SubContent>
</DropdownMenuPortal>
