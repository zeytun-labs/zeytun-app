<script lang="ts">
  import * as Tabs from "$lib/components/ui/tabs/index.js";
  import type { OutboundMode } from "$lib/core/types";
  import { OUTBOUND_MODE_CONFIG } from "$lib/core/constants";
  import { HugeiconsIcon } from "@hugeicons/svelte";
  import {
    CurvyLeftRightDirectionIcon,
    TrendingUpDownIcon,
    CurvyRightDirectionIcon,
    ChevronsLeftRightEllipsisIcon,
  } from "@hugeicons/core-free-icons";
  interface Props {
    mode: OutboundMode;
    onSelectMode: (mode: OutboundMode) => void;
  }

  let { mode, onSelectMode }: Props = $props();
</script>

<div>
  <Tabs.Root value={mode}>
    <Tabs.List variant="primary" class=" justify-start">
      <Tabs.Trigger
        value="direct"
        class="w-40"
        onclick={() => onSelectMode("direct")}
      >
        <HugeiconsIcon icon={ChevronsLeftRightEllipsisIcon} />
        Direct Outbound
      </Tabs.Trigger>
      <Tabs.Trigger
        value="global"
        class="w-40"
        onclick={() => onSelectMode("global")}
      >
        <HugeiconsIcon icon={CurvyRightDirectionIcon} />
        Global Proxy
      </Tabs.Trigger>
      <Tabs.Trigger
        value="rule"
        class="w-40"
        onclick={() => onSelectMode("rule")}
      >
        <HugeiconsIcon icon={TrendingUpDownIcon} />

        Rule-based Proxy
      </Tabs.Trigger>
    </Tabs.List>
  </Tabs.Root>
  <div class="text-muted-foreground/60 rounded-lg px-3 py-1 text-sm">
    {OUTBOUND_MODE_CONFIG[mode].description}
  </div>
</div>
