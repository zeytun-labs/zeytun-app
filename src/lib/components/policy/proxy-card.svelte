<script lang="ts">
  import type { Proxy } from "$lib/core/types";
  import { cn, formatLatency, getLatencyColor } from "$lib/utils";
  import { subscriptionsStore } from "$lib/stores/subscriptions.svelte";
  import { HugeiconsIcon } from "@hugeicons/svelte";
  import { InternetIcon, Link04Icon } from "@hugeicons/core-free-icons";
  interface Props {
    proxy: Proxy;
    allProxies?: Proxy[];
    isSelected: boolean;
    isTesting: boolean;
    latencyMs: number | null | undefined;
    onclick: () => void;
    ondblclick: () => void;
    // Props is passed from ContextMenu.Trigger child snippet
    triggerProps?: Record<string, unknown>;
  }

  let {
    proxy,
    allProxies = [],
    isSelected,
    isTesting,
    latencyMs,
    onclick,
    ondblclick,
    triggerProps = {},
  }: Props = $props();

  // Regional Indicator Symbol pairs (A–Z) → flag emoji in title
  const FLAG_RE = /[\u{1F1E6}-\u{1F1FF}]{2}/u;
  const flagEmoji = $derived(proxy.title.match(FLAG_RE)?.[0] ?? null);
  const displayTitle = $derived(
    flagEmoji
      ? proxy.title
          .replace(FLAG_RE, "")
          .replace(/\s{2,}/g, " ")
          .trim()
      : proxy.title,
  );

  const sourceName = $derived.by(() => {
    if (proxy.origin === "subscription") {
      if (proxy.source_id) {
        const s = subscriptionsStore.sources.find(
          (src) => src.id === proxy.source_id,
        );
        return s?.settings.name ?? "Subscription";
      }
      return "Subscription";
    }
    return "";
  });

  function proxyLabel(tag: string) {
    return allProxies.find((p) => p.tag === tag)?.title || tag;
  }

  const subtitle = $derived(
    proxy.protocol === "chain"
      ? `> ${(proxy.config?.proxies?.slice(-1) ?? []).map(proxyLabel)}`
      : proxy.config
        ? `${proxy.config.address}:${proxy.config.port}`
        : "",
  );
</script>

<button
  {...triggerProps}
  class="group bg-card/70 hover:bg-card border border-border/40 hover:border-border data-[selected=true]:border-primary/30 data-[selected=true]:bg-primary/10 flex min-h-28 w-full flex-col justify-between rounded-4xl p-4 text-left transition-all {!proxy.available
    ? 'opacity-60'
    : ''}"
  data-selected={isSelected}
  type="button"
  {onclick}
  {ondblclick}
>
  <div class="flex items-start justify-between gap-3">
    <div class="flex gap-2 items-center w-full">
      <div
        class="flex size-10 shrink-0 items-center justify-center rounded-xl bg-muted text-muted-foreground transition-colors group-data-[selected=true]:bg-lime-300/10"
      >
        {#if proxy.protocol === "chain"}
          <HugeiconsIcon icon={Link04Icon} class="size-5" />
        {:else if flagEmoji}
          <span class="text-[22px] leading-none drop-shadow-sm"
            >{flagEmoji}</span
          >
        {:else}
          <HugeiconsIcon icon={InternetIcon} class="size-5" />
        {/if}
      </div>
      <div class="min-w-0 flex flex-col flex-1">
        <div
          class="text-muted-foreground text-xs font-mono flex items-center gap-1.5"
        >
          <span>{proxy.protocol.toUpperCase()}</span>
          {#if sourceName.length > 0}
            <span>—</span>
          {/if}
          <span
            class="inline-flex items-center gap-1 truncate text-[11px] max-w-30"
            title={`Source: ${sourceName}`}
          >
            <span class="truncate">{sourceName}</span>
          </span>
          {#if !proxy.available}
            <span
              class="rounded bg-destructive/15 px-1 py-0.2 text-[9px] text-destructive font-sans font-medium"
            >
              Unavailable
            </span>
          {/if}
        </div>
        <div class="truncate text-sm w-full" title={displayTitle}>
          {displayTitle}
        </div>
      </div>
    </div>
  </div>
  <div class="flex items-center justify-between gap-3">
    <span
      class="text-muted-foreground text-xs font-mono truncate"
      title={subtitle}>{subtitle}</span
    >
    {#if isTesting}
      <span
        class="text-muted-foreground text-xs animate-pulse h-5 flex items-end"
        >testing...</span
      >
    {:else}
      <span class={cn("text-sm font-mono", getLatencyColor(latencyMs))}>
        {formatLatency(latencyMs)}
      </span>
    {/if}
  </div>
</button>
