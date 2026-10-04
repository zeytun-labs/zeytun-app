<script lang="ts">
  import DashCard from "./dash-card.svelte";
  import * as Chart from "$lib/components/ui/chart";
  import { formatBytes } from "$lib/utils";
  import { scaleUtc } from "d3-scale";
  import { curveNatural } from "d3-shape";
  import { Area, AreaChart } from "layerchart";

  interface Props {
    title: string;
    data: any[];
    seriesKey: string;
    value: string;
    unit: string;
    color: string;
    chartConfig: any;
  }

  let { title, data, seriesKey, value, unit, color, chartConfig }: Props =
    $props();
</script>

<DashCard class="pt-5">
  <div class="px-5">
    <div
      class="text-muted-foreground text-[11px] font-medium tracking-wider uppercase"
    >
      {title}
    </div>
    <div class="mt-3 flex items-end gap-1.5">
      <span class="text-3xl font-semibold tracking-tight leading-none"
        >{value}</span
      >
      <span class="text-muted-foreground mb-0.5 text-sm font-medium">{unit}</span>
    </div>
  </div>

  <Chart.Container config={chartConfig} class="mt-auto h-24 w-full">
    <AreaChart
      {data}
      x="date"
      xScale={scaleUtc()}
      yPadding={[0, 25]}
      padding={{ bottom: 20 }}
      axis={false}
      grid={false}
      series={[
        {
          key: seriesKey,
          label: title,
          color: color,
        },
      ]}
      seriesLayout="stack"
      props={{
        xAxis: {
          format: (v: Date) =>
            v.toLocaleDateString("en-US", { month: "short" }),
        },
      }}
    >
      {#snippet tooltip()}
        {#snippet bytesFormatter({ value, name, item }: any)}
          <div class="flex w-full items-center gap-2">
            <div
              class="h-2.5 w-2.5 shrink-0 rounded-xs"
              style="background-color: {item.color}"
            ></div>
            <span class="text-muted-foreground lowercase">{name}</span>
            <span class="text-foreground ml-auto font-medium">
              {formatBytes(value as number)}
            </span>
          </div>
        {/snippet}

        <Chart.Tooltip
          indicator="dot"
          formatter={bytesFormatter}
          labelFormatter={(v: Date) =>
            v.toLocaleTimeString("en-US", {
              hour: "2-digit",
              minute: "2-digit",
              second: "2-digit",
              hour12: false,
            })}
        />
      {/snippet}
      {#snippet marks({ context })}
        {#each context.series.visibleSeries as s (s.key)}
          <Area
            {seriesKey}
            curve={curveNatural}
            fillOpacity={0.1}
            line={{ class: "stroke-2" }}
            {...s.props}
            y0={() => context?.yScale?.invert?.(context?.height + 20)}
          />
        {/each}
      {/snippet}
    </AreaChart>
  </Chart.Container>
</DashCard>
