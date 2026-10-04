<script lang="ts">
  import * as Tooltip from "$lib/components/ui/tooltip";
  import type { ConnDto } from "$lib/core/types";
  import { statusColor } from "./lib";

  let { status }: { status: ConnDto["status"] } = $props();
</script>

<Tooltip.Root>
  <Tooltip.Trigger class="p-1">
    <span class="size-2 block shrink-0 rounded-full {statusColor(status)}"></span>
  </Tooltip.Trigger>
  <Tooltip.Content>
    <p class="capitalize">{status}</p>
  </Tooltip.Content>
</Tooltip.Root>
