<script lang="ts">
  import { Button } from "$lib/components/ui/button";
  import { diagnosticsStore } from "$lib/stores/diagnostics.svelte";
  import { profileStore } from "$lib/stores/profile.svelte";
  import { RefreshIcon, Menu01Icon } from "@hugeicons/core-free-icons";
  import { HugeiconsIcon } from "@hugeicons/svelte";
  import { Skeleton } from "$lib/components/ui/skeleton";
  import { Spinner } from "$lib/components/ui/spinner";
  import DiagnosticsSheet from "./diagnostics-sheet.svelte";
  import DashCard from "./dash-card.svelte";
  import { onMount } from "svelte";
  import { cn } from "$lib/utils";

  let sheetOpen = $state(false);

  const loading = $derived(diagnosticsStore.running);
  const result = $derived(diagnosticsStore.result);

  // Proxy box shows N/A unless Global — Rule/Direct don't run a meaningful
  // proxy leg (Rule's probe would traverse the routing table, see lib.rs).
  const isGlobal = $derived(profileStore.outboundMode === "global");

  const fmt = (v: number | null | undefined): string =>
    v === null || v === undefined ? "—" : `${v}`;

  onMount(() => {
    diagnosticsStore.run();
  });

  const metricPill = cn(
    "flex min-w-0 flex-row items-center justify-between gap-0.5 rounded-xl border border-border/30",
    "bg-foreground/5 px-3 py-2",
  );

  const metrics = $derived([
    { label: "Router", value: result?.routerMs },
    { label: "DNS", value: result?.dnsMs },
    {
      label: "Proxy",
      value: isGlobal ? result?.proxyMs : null,
      forceNa: !isGlobal,
    },
  ] as const);
</script>

{#snippet metric(
  label: string,
  value: number | null | undefined,
  forceNa = false,
)}
  <div class={metricPill}>
    <div
      class="text-muted-foreground text-[10px] font-medium tracking-wider uppercase"
    >
      {label}
    </div>
    <div class="flex items-end gap-1 text-sm font-medium">
      {#if forceNa}
        <span class="text-muted-foreground">N/A</span>
      {:else if loading && value == null}
        <Spinner class="size-3.5" />
      {:else if value != null}
        <span>{fmt(value)}</span>
        <span class="text-muted-foreground">ms</span>
      {:else}
        <span class="text-muted-foreground">N/A</span>
      {/if}
    </div>
  </div>
{/snippet}

<DashCard class="justify-between gap-4 p-5">
  <div class="relative flex items-start gap-2">
    <div
      class="text-muted-foreground text-[11px] font-medium tracking-wider uppercase"
    >
      Network Latency
    </div>
    <div class="ms-auto absolute inset-e-0 top-0 flex gap-1.5">
      <Button
        size="icon"
        variant="outline"
        disabled={loading}
        title="Run diagnostics"
        onclick={() => diagnosticsStore.run()}
      >
        {#if loading}
          <Spinner class="size-3.5" />
        {:else}
          <HugeiconsIcon icon={RefreshIcon} />
        {/if}
      </Button>
      <Button
        size="icon"
        variant="outline"
        title="Open diagnostics report"
        onclick={() => (sheetOpen = true)}
      >
        <HugeiconsIcon icon={Menu01Icon} />
      </Button>
    </div>
  </div>

  <!-- Main: Direct Policy latency -->
  <div class="flex items-end gap-1.5">
    {#if loading && result?.directMs == null}
      <Skeleton class="h-10 w-28" />
    {:else if result?.directMs != null}
      <span class="text-4xl font-semibold tracking-tight leading-none">
        {result.directMs}
      </span>
      <span class="text-muted-foreground mb-0.5 text-sm font-medium">ms</span>
    {:else}
      <span class="text-4xl font-semibold tracking-tight text-muted-foreground"
        >N/A</span
      >
    {/if}
  </div>

  <div class="grid grid-cols-3 gap-2">
    {#each metrics as m (m.label)}
      {@render metric(m.label, m.value, "forceNa" in m ? m.forceNa : false)}
    {/each}
  </div>
</DashCard>

<DiagnosticsSheet bind:open={sheetOpen} />
