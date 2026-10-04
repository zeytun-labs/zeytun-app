<script lang="ts">
  import * as Card from "$lib/components/ui/card/index.js";
  import * as Tabs from "$lib/components/ui/tabs/index.js";
  import * as Chart from "$lib/components/ui/chart";
  import { scaleBand } from "d3-scale";
  import { cubicInOut } from "svelte/easing";
  import { BarChart } from "layerchart";

  type TrafficScope = "all" | "proxy";
  type TrafficTab = "client" | "domain" | "policy";

  interface Props {
    chartData: any[];
    chartConfig: any;
    trafficLists: Record<string, { label: string; value: string }[]>;
  }

  let { chartData, chartConfig, trafficLists }: Props = $props();

  let trafficScope = $state<TrafficScope>("all");
  let trafficTab = $state<TrafficTab>("client");
</script>

<Card.Root class="lg:col-span-2 lg:row-span-2">
  <Card.Header class="grid-cols-[1fr_auto]">
    <Card.Title class="text-muted-foreground text-xs font-medium tracking-wide">
      TRAFFIC
    </Card.Title>
    <Tabs.Root bind:value={trafficScope}>
      <Tabs.List class="h-4!">
        <Tabs.Trigger value="all" class="px-3 text-xs">ALL</Tabs.Trigger>
        <Tabs.Trigger value="proxy" class="px-3 text-xs">PROXY</Tabs.Trigger>
      </Tabs.List>
    </Tabs.Root>
  </Card.Header>
  <Card.Content class="flex flex-1 flex-col gap-5">
    <Chart.Container config={chartConfig} class="h-42">
      <BarChart
        data={chartData}
        xScale={scaleBand().padding(0.25)}
        x="month"
        axis="x"
        series={[
          {
            key: "traffic",
            label: "Traffic",
            color: chartConfig.traffic.color,
          },
        ]}
        props={{
          bars: {
            stroke: "none",
            rounded: "all",
            motion: { type: "tween", duration: 500, easing: cubicInOut },
          },
          highlight: { area: { fill: "none" } },
          xAxis: {
            format: (d: string) => {
              const index = chartData.findIndex((item) => item.month === d);
              return index % 4 === 0 ? d : "";
            },
          },
        }}
      >
        {#snippet tooltip()}
          <Chart.Tooltip hideLabel />
        {/snippet}
      </BarChart>
    </Chart.Container>

    <Tabs.Root bind:value={trafficTab} class="gap-3">
      <Tabs.List>
        <Tabs.Trigger value="client" class="px-3 text-xs">CLIENT</Tabs.Trigger>
        <Tabs.Trigger value="domain" class="px-3 text-xs">DOMAIN</Tabs.Trigger>
        <Tabs.Trigger value="policy" class="px-3 text-xs">POLICY</Tabs.Trigger>
      </Tabs.List>

      {#each [
        { value: "client", items: trafficLists.client },
        { value: "domain", items: trafficLists.domain },
        { value: "policy", items: trafficLists.policy },
      ] as tab (tab.value)}
        <Tabs.Content value={tab.value} class="space-y-2">
          {#each tab.items as item (item.label)}
            <div class="flex items-center justify-between rounded-lg bg-muted/45 px-3 py-2">
              <span class="min-w-0 truncate text-sm">{item.label}</span>
              <span class="text-muted-foreground text-sm">{item.value}</span>
            </div>
          {/each}
        </Tabs.Content>
      {/each}
    </Tabs.Root>
  </Card.Content>
</Card.Root>
