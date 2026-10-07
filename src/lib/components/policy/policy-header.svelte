<script lang="ts">
  import { Button } from "$lib/components/ui/button/index.js";
  import * as DropdownMenu from "$lib/components/ui/dropdown-menu/index.js";
  import * as Tabs from "$lib/components/ui/tabs/index.js";
  import Spinner from "$lib/components/ui/spinner/spinner.svelte";
  import {
    Add01Icon,
    ArrowDataTransferHorizontalIcon,
    ClipboardPasteIcon,
    InternetIcon,
    ZapIcon,
    Link04Icon,
    ReloadIcon,
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
    onAddSubscription?: () => void;
    onRefreshAllSubscriptions?: () => void;
    refreshingSubscriptions?: boolean;
    currentTab?: "proxies" | "subscriptions";
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
    onAddSubscription,
    onRefreshAllSubscriptions,
    refreshingSubscriptions = false,
    currentTab = $bindable("proxies"),
    children,
  }: Props = $props();
</script>

<div class="flex items-start justify-between gap-3">
  {@render children?.()}
  <div class="flex items-center gap-2">
    <Tabs.Root bind:value={currentTab}>
      <Tabs.List variant="primary">
        <Tabs.Trigger value="proxies">Proxies & Policies</Tabs.Trigger>
        <Tabs.Trigger value="subscriptions">Subscriptions</Tabs.Trigger>
      </Tabs.List>
    </Tabs.Root>

    {#if currentTab === "subscriptions"}
      <Button
        size="icon"
        variant="outline"
        class="bg-card/70!"
        disabled={refreshingSubscriptions}
        onclick={onRefreshAllSubscriptions}
        title="Refresh all subscriptions"
      >
        {#if refreshingSubscriptions}
          <Spinner class="size-4" />
        {:else}
          <HugeiconsIcon icon={ReloadIcon} />
        {/if}
        <span class="sr-only">Refresh all subscriptions</span>
      </Button>
    {:else}
      <Button
        size="icon"
        variant="outline"
        class="bg-card/70!"
        disabled={testingAll || !isRunning}
        onclick={onTestAll}
        title="Test all proxies"
      >
        <HugeiconsIcon icon={ZapIcon} />
        <span class="sr-only">Test all proxies</span>
      </Button>
    {/if}

    <DropdownMenu.Root>
      <DropdownMenu.Trigger>
        {#snippet child({ props })}
          <Button size="icon" variant="outline" class="bg-card/70!" {...props}>
            <HugeiconsIcon icon={Add01Icon} />
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
        {#if onAddSubscription}
          <DropdownMenu.Item onclick={onAddSubscription}>
            <HugeiconsIcon icon={Link04Icon} class="size-4" />
            Add Subscription
          </DropdownMenu.Item>
        {/if}
        <DropdownMenu.Separator />
        <DropdownMenu.Item onclick={onImportClipboard}>
          <HugeiconsIcon icon={ClipboardPasteIcon} />
          Add from Clipboard
        </DropdownMenu.Item>
      </DropdownMenu.Content>
    </DropdownMenu.Root>
  </div>
</div>
