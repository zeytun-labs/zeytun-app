<script lang="ts">
  import { Button } from "$lib/components/ui/button/index.js";
  import * as DropdownMenu from "$lib/components/ui/dropdown-menu/index.js";
  import {
    Add01Icon,
    ArrowDataTransferHorizontalIcon,
    ClipboardPasteIcon,
    InternetIcon,
    ZapIcon,
    Link04Icon,
  } from "@hugeicons/core-free-icons";
  import { HugeiconsIcon } from "@hugeicons/svelte";

  interface Props {
    testingAll: boolean;
    isRunning: boolean;
    onTestAll: () => void;
    onAddProxy: () => void;
    onAddPolicy: () => void;
    onAddProxyChain: () => void;
    onImportClipboard: () => void;
    children?: import("svelte").Snippet;
  }

  let {
    testingAll,
    isRunning,
    onTestAll,
    onAddProxy,
    onAddPolicy,
    onAddProxyChain,
    onImportClipboard,
    children,
  }: Props = $props();
</script>

<div class="flex justify-between gap-3">
  {@render children?.()}
  <div class="flex gap-2">
    <Button
      size="icon"
      variant="outline"
      class="bg-card/70!"
      disabled={testingAll || !isRunning}
      onclick={onTestAll}
    >
      <!-- <ZapIcon class={cn("size-4", testingAll && "animate-pulse")} /> -->
      <HugeiconsIcon icon={ZapIcon} />
      <span class="sr-only">Test all proxies</span>
    </Button>

    <DropdownMenu.Root>
      <DropdownMenu.Trigger>
        {#snippet child({ props })}
          <Button size="icon" variant="outline" class="bg-card/70!" {...props}>
            <HugeiconsIcon icon={Add01Icon} />

            <!-- <PlusIcon /> -->
            <span class="sr-only">Add policy item</span>
          </Button>
        {/snippet}
      </DropdownMenu.Trigger>
      <DropdownMenu.Content align="end" class="w-52">
        <DropdownMenu.Item onclick={onAddProxy}>
          <HugeiconsIcon icon={InternetIcon} />
          Add new Proxy
        </DropdownMenu.Item>
        <DropdownMenu.Item onclick={onAddPolicy}>
          <HugeiconsIcon icon={ArrowDataTransferHorizontalIcon} />
          Add new Policy
        </DropdownMenu.Item>
        <DropdownMenu.Item onclick={onAddProxyChain}>
          <HugeiconsIcon icon={Link04Icon} class="size-4" />
          Create Proxy Chain
        </DropdownMenu.Item>
        <DropdownMenu.Separator />
        <DropdownMenu.Item onclick={onImportClipboard}>
          <HugeiconsIcon icon={ClipboardPasteIcon} />
          Add from Clipboard
        </DropdownMenu.Item>
      </DropdownMenu.Content>
    </DropdownMenu.Root>
  </div>
</div>
