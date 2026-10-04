<script lang="ts">
  import DashCard from "./dash-card.svelte";
  import * as Tabs from "$lib/components/ui/tabs/index.js";
  import { formatBytes } from "$lib/utils";
  import { coreGetTrafficSummary } from "$lib/core/api";
  import type { TrafficSummary, TrafficPeriod } from "$lib/core/types";

  let period = $state<TrafficPeriod>("today");
  let data = $state<TrafficSummary | null>(null);

  $effect(() => {
    const p = period;
    let cancelled = false;
    coreGetTrafficSummary(p)
      .then((res) => {
        if (!cancelled) data = res;
      })
      .catch(() => {
        if (!cancelled) data = null;
      });
    return () => {
      cancelled = true;
    };
  });

  const direct = $derived(data?.direct_bytes ?? 0);
  const proxy = $derived(data?.proxy_bytes ?? 0);
  const total = $derived(direct + proxy);
  // 0-total: even empty-looking split
  const directPct = $derived(
    total > 0 ? Math.round((direct / total) * 100) : 50,
  );
  const proxyPct = $derived(total > 0 ? Math.round((proxy / total) * 100) : 50);

  const totalFmt = $derived(formatBytes(total));
  const directFmt = $derived(formatBytes(direct));
  const proxyFmt = $derived(formatBytes(proxy));

  const stripe = (color: string) =>
    `repeating-linear-gradient(-45deg, ${color} 0 6px, color-mix(in oklab, ${color} 65%, transparent) 6px 12px)`;
</script>

<DashCard class="justify-between gap-2 p-5">
  <div class="flex items-start gap-2 relative">
    <div
      class="text-muted-foreground text-[11px] font-medium tracking-wider uppercase"
    >
      Total Traffic
    </div>
    <div class="absolute inset-e-0 top-0">
      <Tabs.Root bind:value={period} class="ms-auto">
        <Tabs.List class="h-7! p-0.5!">
          <Tabs.Trigger value="today" class="px-3 text-xs">Today</Tabs.Trigger>
          <Tabs.Trigger value="month" class="px-3 text-xs">Month</Tabs.Trigger>
        </Tabs.List>
      </Tabs.Root>
    </div>
  </div>

  <div class="flex items-end gap-1.5">
    <span class="text-4xl font-semibold tracking-tight leading-none">
      {totalFmt.value}
    </span>
    <span class="text-muted-foreground mb-0.5 text-sm font-medium"
      >{totalFmt.unit}</span
    >
  </div>

  <div class="space-y-3">
    <!-- Proxy green left · Direct orange right (mock order) -->
    <div class="flex h-8 gap-1 overflow-hidden">
      <div
        class="min-w-1 rounded-lg transition-all"
        style="width: {Math.max(
          proxyPct,
          total === 0 ? 50 : 0,
        )}%; background: {stripe('var(--chart-1)')}"
      ></div>
      <div
        class="min-w-1 rounded-lg transition-all"
        style="width: {Math.max(
          directPct,
          total === 0 ? 50 : 0,
        )}%; background: {stripe('var(--chart-2)')}"
      ></div>
    </div>

    <div class="grid grid-cols-2 gap-3">
      <div class="flex items-start gap-1">
        <span class="mt-0.75 size-2 shrink-0 rounded-full bg-chart-1"></span>
        <div class="min-w-0">
          <div
            class="text-muted-foreground text-xs font-medium tracking-wider uppercase"
          >
            Proxy
          </div>
          <div class="truncate text-sm font-medium mt-px">
            {proxyFmt.toString()}
            <span class="text-muted-foreground">· {proxyPct}%</span>
          </div>
        </div>
      </div>
      <div class="flex items-start justify-end gap-2 text-end">
        <div class="min-w-0">
          <div
            class="text-muted-foreground text-xs font-medium tracking-wider uppercase"
          >
            Direct
          </div>
          <div class="truncate text-sm font-medium mt-px">
            {directFmt.toString()}
            <span class="text-muted-foreground">· {directPct}%</span>
          </div>
        </div>
        <span class="mt-0.75 size-2 shrink-0 rounded-full bg-chart-2"></span>
      </div>
    </div>
  </div>
</DashCard>
