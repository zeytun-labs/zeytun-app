<script lang="ts">
  import type { ProxyPolicy } from "$lib/core/types";
  import { profileStore } from "$lib/stores/profile.svelte";
  import {
    Touch06Icon,
    CursorMagicSelection01Icon,
    Rotate02Icon,
    HashIcon,
    StickyNote02Icon,
    SignalNo02Icon,
    WeightScaleIcon,
    JusticeScale02Icon,
    StrategyIcon,
  } from "@hugeicons/core-free-icons";
  import { HugeiconsIcon } from "@hugeicons/svelte";

  interface Props {
    policy: ProxyPolicy;
    isSelected: boolean;
    canSelect: boolean;
    onclick: () => void;
    // Props is passed from ContextMenu.Trigger child snippet
    triggerProps?: Record<string, unknown>;
  }

  let { policy, isSelected, onclick, triggerProps = {} }: Props = $props();
</script>

<button
  {...triggerProps}
  class="group bg-card/70 hover:bg-card border border-border/40 hover:border-border data-[selected=true]:border-primary/30 data-[selected=true]:bg-primary/10 flex min-h-28 w-full flex-col justify-between rounded-4xl p-4 text-left transition-all"
  data-selected={isSelected}
  type="button"
  {onclick}
>
  <div class="flex items-start justify-between gap-3">
    <div class="flex gap-2 items-center w-full">
      <div
        class="flex size-10 shrink-0 items-center justify-center rounded-xl bg-muted text-muted-foreground transition-colors group-data-[selected=true]:bg-lime-300/10"
      >
        {#if policy.kind === "selector"}
          <HugeiconsIcon icon={Touch06Icon} class="size-5" />
        {:else if policy.kind === "urltest"}
          <HugeiconsIcon icon={CursorMagicSelection01Icon} class="size-5" />
        {:else if policy.kind === "balancer" && policy.strategy === "round-robin"}
          <HugeiconsIcon icon={Rotate02Icon} class="size-5" />
        {:else if policy.kind === "balancer" && policy.strategy === "consistent-hashing"}
          <HugeiconsIcon icon={HashIcon} class="size-5" />
        {:else if policy.kind === "balancer" && policy.strategy === "sticky-sessions"}
          <HugeiconsIcon icon={StickyNote02Icon} class="size-5" />
        {:else if policy.kind === "balancer" && policy.strategy === "failover"}
          <HugeiconsIcon icon={SignalNo02Icon} class="size-5" />
        {:else if policy.kind === "balancer" && policy.strategy === "weighted"}
          <HugeiconsIcon icon={WeightScaleIcon} class="size-5" />
        {:else if policy.kind === "balancer" && policy.strategy === "least-connections"}
          <HugeiconsIcon icon={JusticeScale02Icon} class="size-5" />
        {:else}
          <HugeiconsIcon icon={StrategyIcon} class="size-5" />
        {/if}
      </div>
      <div class="min-w-0 flex flex-col">
        <div
          class="text-muted-foreground text-xs font-mono flex items-center gap-1.5 uppercase"
        >
          {policy.kind === "urltest"
            ? "auto"
            : policy.kind === "balancer"
              ? (policy.strategy?.replaceAll("-", " ") ?? "balancer")
              : "manual"}
        </div>
        <div class="truncate text-sm w-full">
          {policy.name}
        </div>
      </div>
    </div>
    <!-- <div class="min-w-0 flex flex-col gap-1">
      <div class="flex items-center gap-1 text-muted-foreground">
        <HugeiconsIcon icon={ArrowDataTransferHorizontalIcon} class="size-4" />
        <span class="text-xs capitalize">
          {policy.kind === "urltest"
            ? "auto"
            : policy.kind === "balancer"
              ? "balancer"
              : "manual"}
        </span>
      </div>
      <div class="truncate text-sm">
        {policy.name}
      </div>
    </div> -->
  </div>
  <div class="flex items-center justify-between gap-3">
    <span class="text-muted-foreground text-xs">
      {#if (policy.kind ?? "selector") === "selector" && policy.selected_member_tag}
        {profileStore.getMemberName(policy.selected_member_tag)}
      {:else}
        {policy.members?.length ?? 0} members
      {/if}
    </span>
  </div>
</button>
