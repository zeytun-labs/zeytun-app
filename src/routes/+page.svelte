<script lang="ts">
  import { onMount } from "svelte";
  import {
    coreGetExternalIpInfo,
    coreGetLocalNetworkInfo,
    type LocalNetworkInfo,
  } from "$lib/core/api";
  import { profileStore } from "$lib/stores/profile.svelte";
  import type * as Chart from "$lib/components/ui/chart";

  import {
    dashboardStore,
    formatSpeed,
  } from "$lib/stores/dashboardStore.svelte";
  import { Skeleton } from "$lib/components/ui/skeleton/index.js";

  import SummaryCards from "$lib/components/dashboard/summary-cards.svelte";
  import ConnectionsCard from "$lib/components/dashboard/connections-card.svelte";
  import NetworkLatencyCard from "$lib/components/dashboard/network-latency-card.svelte";
  import DashCard from "$lib/components/dashboard/dash-card.svelte";
  import TrafficSummaryCard from "$lib/components/dashboard/traffic-summary-card.svelte";

  // Live active-connection stats from the daemon traffic stream.
  const activeConnections = $derived(
    dashboardStore.processes.reduce((sum, p) => sum + p.activeConnections, 0),
  );

  // Dynamically import heavy chart components to prevent main thread blocking on navigation
  type SpeedChartComponent =
    typeof import("$lib/components/dashboard/speed-chart.svelte")["default"];
  type TrafficCardComponent =
    typeof import("$lib/components/dashboard/traffic-analytics-card.svelte")["default"];
  let AsyncSpeedChart = $state<Promise<SpeedChartComponent> | null>(null);
  let AsyncTrafficCard = $state<Promise<TrafficCardComponent> | null>(null);

  const speedChartConfig = {
    upload: { label: "Upload", color: "var(--chart-2)" },
    download: { label: "Download", color: "var(--chart-1)" },
  } satisfies Chart.ChartConfig;

  let externalIp = $state<{ ip: string; flag: string } | null>(null);
  let externalIpLoading = $state(true);
  let localNetworkInfo = $state<LocalNetworkInfo | null>(null);

  onMount(() => {
    const loadAll = Promise.all([
      import("$lib/components/dashboard/speed-chart.svelte").then(
        (m) => m.default,
      ),
      import("$lib/components/dashboard/traffic-analytics-card.svelte").then(
        (m) => m.default,
      ),
    ]);
    AsyncSpeedChart = loadAll.then((res) => res[0]);
    AsyncTrafficCard = loadAll.then((res) => res[1]);

    coreGetExternalIpInfo()
      .then(({ ip, emoji }) => {
        externalIp = { ip, flag: emoji };
      })
      .catch((e) => {
        console.error("Failed to fetch IP:", e);
        externalIp = null;
      })
      .finally(() => {
        externalIpLoading = false;
      });

    coreGetLocalNetworkInfo()
      .then((info) => {
        localNetworkInfo = info;
      })
      .catch((e) => {
        console.error("Failed to fetch local network info:", e);
        localNetworkInfo = null;
      });
  });
</script>

{#snippet speedSkeleton()}
  <DashCard class="pt-5">
    <div class="shrink-0 px-5">
      <Skeleton class="h-4 w-16" />
      <Skeleton class="mt-3 h-7.5 w-24" />
    </div>
    <div class="mt-auto h-28 w-full shrink-0 px-5 pb-3 flex items-end">
      <Skeleton class="h-20 w-full rounded-lg" />
    </div>
  </DashCard>
{/snippet}

{#snippet trafficSkeleton()}
  <DashCard class="gap-2 p-4">
    <div class="flex h-7 shrink-0 items-center justify-between">
      <Skeleton class="h-3.5 w-16" />
      <div class="flex items-center gap-1.5">
        <Skeleton class="h-7 w-20 rounded-xl" />
        <Skeleton class="h-7 w-36 rounded-xl" />
      </div>
    </div>
    <Skeleton class="h-36 w-full shrink-0 rounded-xl" />
    <div class="flex h-50.5 shrink-0 flex-col gap-1.5">
      <div class="flex h-7 shrink-0 items-center justify-between">
        <Skeleton class="h-7 w-48 rounded-xl" />
        <div class="flex items-center gap-3">
          <Skeleton class="h-3 w-16" />
          <Skeleton class="h-3 w-16" />
        </div>
      </div>
      <div class="h-42 shrink-0 flex flex-col">
        {#each Array.from({ length: 6 }) as _, i (i)}
          <div class="flex h-7 shrink-0 items-center px-2.5">
            <Skeleton class="h-4 w-full" />
          </div>
        {/each}
      </div>
    </div>
  </DashCard>
{/snippet}

<!-- Reserve tracks before onMount/imports. h-full only fills a track; it cannot
     keep an auto track from resizing when its intrinsic content changes.
     The detail pair includes the 0.75rem gap: 26.75rem (428px) for Traffic.
     Its content uses 28px header + 144px chart + 202px tabs/list + 16px gaps
     + 34px padding/borders. The 168px list fits six 28px rows; extra entries
     scroll inside it. The grid is 692px tall at the default root size. -->
<div
  class="grid flex-1 grid-cols-4 grid-rows-[2.5rem_12.5rem_13rem_13rem] gap-3 content-start *:min-h-0 *:min-w-0"
>
  <div class="grid grid-cols-4 gap-3 col-span-4">
    <SummaryCards
      {externalIp}
      {externalIpLoading}
      outboundMode={profileStore.outboundMode}
      {localNetworkInfo}
    />
  </div>

  <div class="col-span-2">
    <ConnectionsCard
      headline={String(activeConnections)}
      process={dashboardStore.processes.length}
    />
  </div>

  {#each ["upload", "download"] as seriesKey (seriesKey)}
    <div class="col-span-1">
      {#if AsyncSpeedChart}
        {#await AsyncSpeedChart}
          {@render speedSkeleton()}
        {:then SpeedChart}
          {@const speed = formatSpeed(
            seriesKey === "upload"
              ? dashboardStore.globalUploadSpeed
              : dashboardStore.globalDownloadSpeed,
          )}
          <SpeedChart
            title={seriesKey.toUpperCase()}
            data={dashboardStore.speedHistory}
            {seriesKey}
            value={speed.value}
            unit={speed.unit}
            color={seriesKey === "upload"
              ? "var(--color-upload)"
              : "var(--color-download)"}
            chartConfig={speedChartConfig}
          />
        {/await}
      {:else}
        {@render speedSkeleton()}
      {/if}
    </div>
  {/each}

  <div class="col-span-2">
    <NetworkLatencyCard />
  </div>

  <div class="col-span-2 row-span-2">
    {#if AsyncTrafficCard}
      {#await AsyncTrafficCard}
        {@render trafficSkeleton()}
      {:then TrafficCard}
        <TrafficCard />
      {/await}
    {:else}
      {@render trafficSkeleton()}
    {/if}
  </div>

  <div class="col-span-2">
    <TrafficSummaryCard />
  </div>
</div>
