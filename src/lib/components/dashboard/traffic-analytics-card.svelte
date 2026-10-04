<script lang="ts">
  import DashCard from "./dash-card.svelte";
  import * as Tabs from "$lib/components/ui/tabs/index.js";
  import { formatBytes } from "$lib/utils";
  import { coreGetTrafficAnalytics } from "$lib/core/api";
  import { dashboardStore } from "$lib/stores/dashboardStore.svelte";
  import { profileStore } from "$lib/stores/profile.svelte";
  import ProcessIcon from "../../../routes/process/components/process-icon.svelte";

  import { scaleBand } from "d3-scale";
  import { BarChart, Highlight } from "layerchart";
  import { cubicInOut } from "svelte/easing";
  import * as Chart from "$lib/components/ui/chart/index.js";

  import type {
    TrafficAnalytics,
    TrafficFilter,
    TrafficCategory,
    TrafficTopEntry,
    TrafficRange,
  } from "$lib/core/types";

  const RANGES: { value: TrafficRange; label: string }[] = [
    { value: "1h", label: "1H" },
    { value: "6h", label: "6H" },
    { value: "24h", label: "24H" },
    { value: "7d", label: "7D" },
    { value: "30d", label: "30D" },
  ];

  function rangeSpec(range: TrafficRange): {
    bucketSecs: number;
    count: number;
    kind: "minute" | "hour" | "day";
  } {
    switch (range) {
      case "1h":
        return { bucketSecs: 60, count: 60, kind: "minute" };
      case "6h":
        return { bucketSecs: 5 * 60, count: 72, kind: "minute" };
      case "24h":
        return { bucketSecs: 3600, count: 24, kind: "hour" };
      case "7d":
        return { bucketSecs: 24 * 3600, count: 7, kind: "day" };
      case "30d":
        return { bucketSecs: 24 * 3600, count: 30, kind: "day" };
      default:
        return { bucketSecs: 3600, count: 24, kind: "hour" };
    }
  }

  let topFilter = $state<TrafficFilter>("all");
  let range = $state<TrafficRange>("24h");
  let bottomTab = $state<TrafficCategory>("domain");
  let data = $state<TrafficAnalytics | null>(null);
  let loading = $state(true);
  let refreshTick = $state(0);

  let currentTime = $state(Date.now());
  $effect(() => {
    const clock = setInterval(() => {
      currentTime = Date.now();
    }, 60_000);
    const refresh = setInterval(() => {
      refreshTick++;
    }, 60_000);
    return () => {
      clearInterval(clock);
      clearInterval(refresh);
    };
  });

  $effect(() => {
    const filter = topFilter;
    const r = range;
    void refreshTick;
    let cancelled = false;
    loading = true;
    coreGetTrafficAnalytics(filter, r)
      .then((res: TrafficAnalytics) => {
        if (!cancelled) {
          data = res;
          loading = false;
        }
      })
      .catch(() => {
        if (!cancelled) loading = false;
      });
    return () => {
      cancelled = true;
    };
  });

  const buckets = $derived.by(() => {
    const { bucketSecs, count, kind } = rangeSpec(range);
    const currentBucketTs =
      Math.floor(currentTime / 1000 / bucketSecs) * bucketSecs;

    const out = Array.from({ length: count }, (_, i) => {
      const ago = count - 1 - i;
      const bucketTs = currentBucketTs - ago * bucketSecs;
      const dateObj = new Date(bucketTs * 1000);

      let label: string;
      let tooltipLabel: string;

      if (kind === "day") {
        label = dateObj.toLocaleDateString("en-US", {
          month: "short",
          day: "numeric",
        });
        tooltipLabel = dateObj.toLocaleDateString("en-US", {
          weekday: "short",
          month: "short",
          day: "numeric",
        });
      } else {
        const time = dateObj.toLocaleTimeString("en-US", {
          hour: "2-digit",
          minute: "2-digit",
          hour12: false,
        });
        const now = new Date(currentTime);
        const isToday = dateObj.toDateString() === now.toDateString();
        const dayLabel = isToday ? "Today" : "Yesterday";
        label = ago === 0 ? "now" : time;
        tooltipLabel = `${dayLabel}, ${time}`;
      }

      return { label, tooltipLabel, up: 0, down: 0, ts: bucketTs };
    });

    if (!data) return out;

    for (const p of data.series) {
      const idx = count - 1 - Math.floor((currentBucketTs - p.ts) / bucketSecs);
      if (idx >= 0 && idx < count) {
        out[idx].up += p.up;
        out[idx].down += p.down;
      }
    }

    return out;
  });

  const topList = $derived<TrafficTopEntry[]>(
    bottomTab === "domain"
      ? (data?.top_domains ?? [])
      : bottomTab === "policy"
        ? (data?.top_policies ?? [])
        : (data?.top_processes ?? []),
  );

  const windowTotal = $derived(buckets.reduce((s, b) => s + b.up + b.down, 0));

  function sharePct(entry: TrafficTopEntry): number {
    if (windowTotal <= 0) return 0;
    return Math.min(100, ((entry.up + entry.down) / windowTotal) * 100);
  }

  function displayName(rawName: string): string {
    return bottomTab === "policy"
      ? profileStore.getMemberName(rawName)
      : rawName;
  }

  function processPath(name: string): string {
    return dashboardStore.processes.find((p) => p.name === name)?.path ?? "";
  }

  const chartConfig = {
    up: { label: "Upload", color: "var(--chart-2)" },
    down: { label: "Download", color: "var(--chart-1)" },
  } satisfies Chart.ChartConfig;

  const xTicks = $derived.by(() => {
    const n = buckets.length;
    let step = 3;
    if (range === "6h") step = 8;
    else if (range === "7d") step = 1;
    else if (n <= 24) step = 3;
    else if (n === 30) step = 4;
    else if (n <= 72) step = 6;
    else step = Math.ceil(n / 8);

    return buckets.filter((_, i) => (i + 1) % step === 0).map((b) => b.label);
  });
</script>

<DashCard class="gap-4 p-5">
  <div class="flex flex-wrap items-center gap-2 relative">
    <div
      class="text-muted-foreground text-[11px] font-medium tracking-wider uppercase"
    >
      Traffic
    </div>
    <div
      class="ms-auto flex flex-wrap items-center justify-end gap-1.5 absolute inset-e-0 top-0"
    >
      <Tabs.Root bind:value={topFilter}>
        <Tabs.List class="h-7! p-0.5!">
          <Tabs.Trigger value="all" class="px-2.5 text-xs">All</Tabs.Trigger>
          <Tabs.Trigger value="proxy" class="px-2.5 text-xs">Proxy</Tabs.Trigger
          >
        </Tabs.List>
      </Tabs.Root>
      <Tabs.Root bind:value={range}>
        <Tabs.List class="h-7! p-0.5!">
          {#each RANGES as r (r.value)}
            <Tabs.Trigger value={r.value} class="px-2 text-xs">
              {r.label}
            </Tabs.Trigger>
          {/each}
        </Tabs.List>
      </Tabs.Root>
    </div>
  </div>

  <!-- REQUIRED: existing Chart + BarChart stack (up/down) -->
  <Chart.Container config={chartConfig} class="h-40 w-full">
    <BarChart
      data={buckets}
      xScale={scaleBand().padding(0.25)}
      x="label"
      axis="x"
      rule={false}
      series={[
        {
          key: "up",
          label: "Upload",
          color: "url(#stripe-up)",
          props: { rounded: "bottom", radius: 10 },
        },
        {
          key: "down",
          label: "Download",
          color: "url(#stripe-down)",
          props: { rounded: "top", radius: 10 },
        },
      ]}
      seriesLayout="stack"
      props={{
        bars: {
          stroke: "none",
          motion: { type: "tween", duration: 500, easing: cubicInOut },
        },
        highlight: { area: false },
        xAxis: {
          ticks: xTicks,
        },
      }}
    >
      {#snippet belowMarks()}
        <defs>
          <pattern id="stripe-up" patternUnits="userSpaceOnUse" width="12" height="12" patternTransform="rotate(-45)">
            <rect width="6" height="12" fill="var(--chart-2)" />
            <rect x="6" width="6" height="12" fill="color-mix(in oklab, var(--chart-2) 65%, transparent)" />
          </pattern>
          <pattern id="stripe-down" patternUnits="userSpaceOnUse" width="12" height="12" patternTransform="rotate(-45)">
            <rect width="6" height="12" fill="var(--chart-1)" />
            <rect x="6" width="6" height="12" fill="color-mix(in oklab, var(--chart-1) 65%, transparent)" />
          </pattern>
        </defs>
        <Highlight area={{ class: "fill-muted" }} />
      {/snippet}
      {#snippet tooltip()}
        {#snippet bytesFormatter({ value, name, item }: any)}
          <div class="flex w-full items-center gap-2">
            <div
              class="h-2.5 w-2.5 shrink-0 rounded-xs"
              style="background-color: {item.color}"
            ></div>
            <span class="text-muted-foreground">{name}</span>
            <span class="text-foreground ml-auto font-medium">
              {formatBytes(value as number)}
            </span>
          </div>
        {/snippet}

        <Chart.Tooltip
          formatter={bytesFormatter}
          labelFormatter={(label: string) =>
            buckets.find((b) => b.label === label)?.tooltipLabel ?? label}
        />
      {/snippet}
    </BarChart>
  </Chart.Container>

  <Tabs.Root bind:value={bottomTab} class="gap-2">
    <div class="flex items-center justify-between">
      <Tabs.List class="h-7! p-0.5!">
        <Tabs.Trigger value="domain" class="flex-1 text-xs">Host</Tabs.Trigger>
        <Tabs.Trigger value="policy" class="flex-1 text-xs">Policy</Tabs.Trigger
        >
        <Tabs.Trigger value="process" class="flex-1 text-xs"
          >Process</Tabs.Trigger
        >
      </Tabs.List>
      <div class="justify-center flex items-center gap-4 text-xs">
        <div class="flex items-center gap-1.5">
          <span class="size-2 rounded-full bg-chart-2"></span>
          <span
            class="text-muted-foreground font-medium tracking-wider uppercase"
            >Upload</span
          >
        </div>
        <div class="flex items-center gap-1.5">
          <span class="size-2 rounded-full bg-chart-1"></span>
          <span
            class="text-muted-foreground font-medium tracking-wider uppercase"
            >Download</span
          >
        </div>
      </div>
    </div>

    <div class="h-39">
      {#if loading && !data}
        <div class="text-muted-foreground py-6 text-center text-xs">
          Loading…
        </div>
      {:else if topList.length === 0}
        <div class="text-muted-foreground py-6 text-center text-xs">
          No traffic yet.
        </div>
      {:else}
        {#each topList as entry (entry.name)}
          {@const pct = sharePct(entry)}
          <div
            class="flex items-center justify-between gap-2 rounded-xl px-2.5 py-1.5 hover:bg-foreground/5"
          >
            <span class="flex min-w-0 items-center gap-2">
              {#if bottomTab === "process"}
                <ProcessIcon
                  name={entry.name}
                  path={processPath(entry.name)}
                  class="size-4 shrink-0"
                />
              {/if}
              <span
                class="truncate text-sm {bottomTab === 'domain'
                  ? 'font-mono'
                  : ''}"
                title={displayName(entry.name)}
              >
                {displayName(entry.name)}
              </span>
            </span>
            <span class="text-muted-foreground shrink-0 text-xs tabular-nums">
              <span class="text-chart-2"
                >↑ {formatBytes(entry.up).toString()}</span
              >
              <span class="mx-1 opacity-40">/</span>
              <span class="text-chart-1"
                >↓ {formatBytes(entry.down).toString()}</span
              >
              <span class="text-muted-foreground/60 ml-1.5"
                >{pct.toFixed(0)}%</span
              >
            </span>
          </div>
        {/each}
      {/if}
    </div>
  </Tabs.Root>
</DashCard>
