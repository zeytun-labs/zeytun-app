<script lang="ts">
  import { Skeleton } from "$lib/components/ui/skeleton/index.js";
  import type { OutboundMode } from "$lib/core/types";
  import type { LocalNetworkInfo } from "$lib/core/api";
  import {
    Wifi02Icon,
    SmartPhone01Icon,
    ZapIcon,
    PlugSocketIcon,
    EthernetPortIcon,
    UnavailableIcon,
    DoorOpenIcon,
  } from "@hugeicons/core-free-icons";
  import { HugeiconsIcon } from "@hugeicons/svelte";
  import DashCard from "./dash-card.svelte";
  import Spinner from "../ui/spinner/spinner.svelte";

  interface Props {
    externalIp: { ip: string; flag: string } | null;
    externalIpLoading: boolean;
    outboundMode: OutboundMode;
    localNetworkInfo?: LocalNetworkInfo | null;
  }

  let { externalIp, externalIpLoading, outboundMode, localNetworkInfo }: Props = $props();

  const networkIcon = $derived.by((): typeof Wifi02Icon => {
    const t = localNetworkInfo?.networkType?.toLowerCase?.();
    if (t === "wifi" || t === "wi-fi") return Wifi02Icon;
    if (t === "cellular") return SmartPhone01Icon;
    if (t === "thunderbolt") return ZapIcon;
    return PlugSocketIcon;
  });

  const outboundLabel = $derived(
    outboundMode === "rule"
      ? "Rule-Based Proxy"
      : outboundMode === "global"
        ? "Global Proxy"
        : "Direct Connection",
  );

  const iconWrap =
    "flex size-10 shrink-0 items-center justify-center rounded-xl bg-card text-foreground";
</script>

{#snippet iconCard(
  label: string,
  value: string | null,
  icon: typeof Wifi02Icon | null,
  flag?: string | null,
  loading = value === null,
)}
  <DashCard
    class="min-w-0 flex-row items-center gap-4 px-3 bg-transparent! border-0"
  >
    <div class={iconWrap}>
      {#if flag}
        <span class="text-xl leading-none">{flag}</span>
      {:else if icon}
        {#key icon}
          <HugeiconsIcon {icon} class="size-5!" />
        {/key}
      {:else if loading}
        <Spinner />
      {:else}
        <HugeiconsIcon icon={UnavailableIcon} class="size-5!" />
      {/if}
    </div>
    <div class="min-w-0 flex-1">
      <div
        class="text-muted-foreground text-[11px] font-medium tracking-wider uppercase"
      >
        {label}
      </div>
      {#if loading}
        <Skeleton class="h-4 mt-1 w-2/3" />
      {:else}
        <div class="truncate text-sm font-medium text-foreground">{value ?? "Unavailable"}</div>
      {/if}
    </div>
  </DashCard>
{/snippet}

{@render iconCard("Network", localNetworkInfo?.name ?? null, networkIcon)}
{@render iconCard(
  "Local IP",
  localNetworkInfo?.localIp ?? null,
  EthernetPortIcon,
)}
{@render iconCard("Outbound Mode", outboundLabel, DoorOpenIcon)}
{@render iconCard(
  "External IP",
  externalIp?.ip ?? null,
  null,
  externalIp?.flag ?? null,
  externalIpLoading,
)}
