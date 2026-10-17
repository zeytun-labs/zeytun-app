<script lang="ts">
  import type { ComponentProps } from "svelte";
  import { HugeiconsIcon } from "@hugeicons/svelte";
  import { Search01Icon } from "@hugeicons/core-free-icons";
  import { Spinner } from "$lib/components/ui/spinner";

  let { loading, icon = Search01Icon }: {
    loading: boolean;
    icon?: ComponentProps<typeof HugeiconsIcon>["icon"];
  } = $props();
</script>

<span aria-hidden="true" class="relative inline-block size-4 shrink-0">
  <span class={`absolute inset-0 inline-flex items-center justify-center transition-[opacity,translate] duration-150 ease-out motion-reduce:translate-y-0 motion-reduce:transition-none ${loading ? "translate-y-0.5 opacity-0" : "translate-y-0 opacity-100"}`}>
    <HugeiconsIcon {icon} strokeWidth={2} class="size-4" />
  </span>
  <span class={`absolute inset-0 inline-flex items-center justify-center transition-[opacity,translate] duration-150 ease-out motion-reduce:translate-y-0 motion-reduce:transition-none ${loading ? "translate-y-0 opacity-100" : "-translate-y-0.5 opacity-0"}`}>
    <Spinner role="presentation" class={`size-4 border-current/25 border-t-current motion-reduce:animate-none ${loading ? "" : "[animation-play-state:paused]"}`} />
  </span>
</span>
