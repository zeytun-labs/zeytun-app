<script lang="ts">
	import { Toaster as Sonner, type ToasterProps as SonnerProps } from "svelte-sonner";
	import { mode } from "mode-watcher";
	import { HugeiconsIcon } from "@hugeicons/svelte"
	import { Loading03Icon } from '@hugeicons/core-free-icons';
	import { CheckmarkCircle02Icon } from '@hugeicons/core-free-icons';
	import { MultiplicationSignCircleIcon } from '@hugeicons/core-free-icons';
	import { InformationCircleIcon } from '@hugeicons/core-free-icons';
	import { Alert02Icon } from '@hugeicons/core-free-icons';

	let { ...restProps }: SonnerProps = $props();
</script>

<!--
	Castle glass toast: inline vars on the toaster root override svelte-sonner's
	theme defaults (inline style beats the [data-sonner-theme] stylesheet rules).
	--normal-bg is translucent card + backdrop-blur from the :global block below
	= same glass chrome as DashCard (bg-card/70 + border-border/40 + blur).
-->
<Sonner
	theme={mode.current}
	class="toaster group"
	style="--border-radius: 1rem; --normal-bg: color-mix(in oklab, var(--color-card) 72%, transparent); --normal-text: var(--color-card-foreground); --normal-border: color-mix(in oklab, var(--color-border) 60%, transparent); --normal-bg-hover: color-mix(in oklab, var(--color-card) 85%, transparent);"
	{...restProps}
>
	{#snippet loadingIcon()}
		<HugeiconsIcon icon={Loading03Icon} strokeWidth={2} class="size-4 animate-spin text-muted-foreground" />
	{/snippet}
	{#snippet successIcon()}
		<HugeiconsIcon icon={CheckmarkCircle02Icon} strokeWidth={2} class="size-4 text-primary" />
	{/snippet}
	{#snippet errorIcon()}
		<HugeiconsIcon icon={MultiplicationSignCircleIcon} strokeWidth={2} class="size-4 text-destructive" />
	{/snippet}
	{#snippet infoIcon()}
		<HugeiconsIcon icon={InformationCircleIcon} strokeWidth={2} class="size-4 text-muted-foreground" />
	{/snippet}
	{#snippet warningIcon()}
		<HugeiconsIcon icon={Alert02Icon} strokeWidth={2} class="size-4 text-amber-500" />
	{/snippet}
</Sonner>

<style>
	/* beats the component's own [data-styled='true'] rule via higher specificity */
	:global([data-sonner-toaster] [data-sonner-toast][data-styled="true"]) {
		backdrop-filter: blur(24px) saturate(1.5);
		box-shadow: 0 8px 32px -4px rgb(0 0 0 / 0.15);
	}

	:global([data-sonner-toaster] [data-sonner-toast][data-styled="true"] [data-button]),
	:global([data-sonner-toaster] [data-sonner-toast][data-styled="true"] [data-cancel]) {
		border-radius: 9999px;
		height: 26px;
		padding-left: 12px;
		padding-right: 12px;
		font-size: 12px;
		font-weight: 500;
		transition: opacity 150ms ease, transform 100ms ease;
	}

	:global([data-sonner-toaster] [data-sonner-toast][data-styled="true"] [data-button]:hover) {
		opacity: 0.9;
	}

	:global([data-sonner-toaster] [data-sonner-toast][data-styled="true"] [data-button]:active) {
		transform: scale(0.97);
	}
</style>
