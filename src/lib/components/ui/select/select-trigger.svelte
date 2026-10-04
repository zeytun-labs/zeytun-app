<script lang="ts" module>
	import { Select as SelectPrimitive } from "bits-ui";
	import { cn, type WithoutChild } from "$lib/utils.js";
	import { HugeiconsIcon } from "@hugeicons/svelte";
	import { UnfoldMoreIcon } from "@hugeicons/core-free-icons";
	import { type VariantProps, tv } from "tailwind-variants";

	export const selectTriggerVariants = tv({
		base: "data-placeholder:text-muted-foreground focus-visible:border-ring focus-visible:ring-ring/30 aria-invalid:ring-destructive/20 dark:aria-invalid:ring-destructive/40 aria-invalid:border-destructive dark:aria-invalid:border-destructive/50 gap-1.5 rounded-3xl border px-3 py-2 text-sm transition-[color,box-shadow,background-color] focus-visible:ring-3 aria-invalid:ring-3 data-[size=default]:h-9 data-[size=sm]:h-8 *:data-[slot=select-value]:flex *:data-[slot=select-value]:gap-1.5 [&_svg:not([class*='size-'])]:size-4 flex w-fit items-center justify-between whitespace-nowrap outline-none disabled:cursor-not-allowed disabled:opacity-50 *:data-[slot=select-value]:line-clamp-1 *:data-[slot=select-value]:flex *:data-[slot=select-value]:items-center [&_svg]:pointer-events-none [&_svg]:shrink-0",
		variants: {
			variant: {
				default: "bg-input/50 border-transparent",
				backless: "bg-card/70 border-border/40",
			},
		},
		defaultVariants: {
			variant: "default",
		},
	});

	export type SelectTriggerVariant = VariantProps<typeof selectTriggerVariants>["variant"];

	export type SelectTriggerProps = WithoutChild<SelectPrimitive.TriggerProps> & {
		size?: "sm" | "default";
		variant?: SelectTriggerVariant;
	};
</script>

<script lang="ts">
	let {
		ref = $bindable(null),
		class: className,
		children,
		size = "default",
		variant = "default",
		...restProps
	}: SelectTriggerProps = $props();
</script>

<SelectPrimitive.Trigger
	bind:ref
	data-slot="select-trigger"
	data-size={size}
	class={cn(selectTriggerVariants({ variant }), className)}
	{...restProps}
>
	{@render children?.()}
	<HugeiconsIcon icon={UnfoldMoreIcon} strokeWidth={2} class="text-muted-foreground size-4 pointer-events-none" />
</SelectPrimitive.Trigger>
