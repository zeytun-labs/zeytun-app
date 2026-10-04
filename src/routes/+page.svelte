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

  import TrafficSummaryCard from "$lib/components/dashboard/traffic-summary-card.svelte";

  // Live active-connection stats from the daemon traffic stream.
  const activeConnections = $derived(
    dashboardStore.processes.reduce((sum, p) => sum + p.activeConnections, 0),
  );

  // Dynamically import heavy chart components to prevent main thread blocking on navigation
  let AsyncSpeedChart = $state<Promise<any> | null>(null);
  let AsyncTrafficCard = $state<Promise<any> | null>(null);

  const speedChartConfig = {
    upload: { label: "Upload", color: "var(--chart-2)" },
    download: { label: "Download", color: "var(--chart-1)" },
  } satisfies Chart.ChartConfig;

  const trafficChartConfig = {
    traffic: { label: "Traffic", color: "var(--chart-1)" },
  } satisfies Chart.ChartConfig;

  let externalIp = $state<{ ip: string; flag: string } | null>(null);
  let externalIpLoading = $state(true);
  let localNetworkInfo = $state<LocalNetworkInfo | null>(null);

  onMount(async () => {
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

<div class="grid flex-1 grid-cols-4 gap-4 content-start">
  <div class="grid grid-cols-4 gap-4 col-span-4 h-fit">
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

  {#if AsyncSpeedChart}
    {#await AsyncSpeedChart}
      <Skeleton class="col-span-1 h-45.75 w-full" />
      <Skeleton class="col-span-1 h-45.75 w-full" />
    {:then SpeedChart}
      {@const upFmt = formatSpeed(dashboardStore.globalUploadSpeed)}
      <div class="col-span-1">
        <SpeedChart
          title="UPLOAD"
          data={dashboardStore.speedHistory}
          seriesKey="upload"
          value={upFmt.value}
          unit={upFmt.unit}
          color="var(--color-upload)"
          chartConfig={speedChartConfig}
        />
      </div>

      {@const downFmt = formatSpeed(dashboardStore.globalDownloadSpeed)}
      <div class="col-span-1">
        <SpeedChart
          title="DOWNLOAD"
          data={dashboardStore.speedHistory}
          seriesKey="download"
          value={downFmt.value}
          unit={downFmt.unit}
          color="var(--color-download)"
          chartConfig={speedChartConfig}
        />
      </div>
    {/await}
  {:else}
    <Skeleton class="col-span-1 h-45.75 w-full" />
    <Skeleton class="col-span-1 h-45.75 w-full" />
  {/if}

  <div class="col-span-2">
    <NetworkLatencyCard />
  </div>

  <div class="col-span-2 row-span-2">
    {#if AsyncTrafficCard}
      {#await AsyncTrafficCard}
        <Skeleton class="h-full w-full min-h-108.25" />
      {:then TrafficCard}
        <TrafficCard />
      {/await}
    {:else}
      <Skeleton class="h-full w-full min-h-108.25" />
    {/if}
    <!-- <TrafficAnalyticsCard /> -->
  </div>

  <div class="col-span-2">
    <TrafficSummaryCard />
  </div>
</div>
