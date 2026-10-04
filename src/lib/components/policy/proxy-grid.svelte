<script lang="ts">
  import { writeText } from "@tauri-apps/plugin-clipboard-manager";
  import * as ContextMenu from "$lib/components/ui/context-menu/index.js";
  import { Button } from "$lib/components/ui/button/index.js";
  import ProxyCard from "./proxy-card.svelte";
  import ProxyQrDialog from "./proxy-qr-dialog.svelte";
  import type { Proxy } from "$lib/core/types";
  import { latencyStore } from "$lib/stores/latency.svelte";
  import { profileStore } from "$lib/stores/profile.svelte";
  import { coreProxyExportLink } from "$lib/core/api";
  import { toast } from "svelte-sonner";
  import { HugeiconsIcon } from "@hugeicons/svelte";
  import {
    Add01Icon,
    CheckIcon,
    Copy01Icon,
    Delete01Icon,
    Edit01Icon,
    QrCodeIcon,
    ZapIcon,
  } from "@hugeicons/core-free-icons";
  import { errorMessage } from "$lib/errors";

  interface Props {
    proxies: Proxy[];
    canSelectProxy: boolean;
    groupName: string;
    onProxyClick: (proxy: Proxy) => void;
    onEditProxy: (proxy: Proxy) => void;
    onSelectProxy: (tag: string) => void;
    onTestProxy: (proxy: Proxy) => void;
    onDeleteProxy: (proxy: Proxy) => void;
    onAddProxy: () => void;
  }

  let {
    proxies,
    canSelectProxy,
    groupName,
    onProxyClick,
    onEditProxy,
    onSelectProxy,
    onTestProxy,
    onDeleteProxy,
    onAddProxy,
  }: Props = $props();

  let qrOpen = $state(false);
  let qrLink = $state("");
  let qrTitle = $state("");

  function isDefaultMemberSelected(memberTag: string) {
    if (!canSelectProxy) return false;
    const defaultPolicy = profileStore.getDefaultPolicy();
    return defaultPolicy?.selected_member_tag === memberTag;
  }

  async function exportLink(proxy: Proxy): Promise<string | null> {
    try {
      return await coreProxyExportLink(proxy.tag);
    } catch (e) {
      toast.error(errorMessage(e));
      return null;
    }
  }

  async function copyShareLink(proxy: Proxy) {
    const link = await exportLink(proxy);
    if (!link) return;
    try {
      await writeText(link);
      toast.success("Link copied.");
    } catch (e) {
      toast.error(errorMessage(e));
    }
  }

  async function showQr(proxy: Proxy) {
    const link = await exportLink(proxy);
    if (!link) return;
    qrLink = link;
    qrTitle = proxy.title || proxy.tag;
    qrOpen = true;
  }
</script>

<div class="flex flex-col gap-2">
  <p class="text-muted-foreground text-xs tracking-wide">PROXY</p>
  <div class="grid gap-3 grid-cols-2 lg:grid-cols-4">
    {#each proxies as proxy (proxy.tag)}
      <ContextMenu.Root>
        <ContextMenu.Trigger>
          {#snippet child({ props })}
            <ProxyCard
              {proxy}
              allProxies={proxies}
              isSelected={isDefaultMemberSelected(proxy.tag)}
              isTesting={latencyStore.isTestingProxy(proxy.tag)}
              latencyMs={latencyStore.getLatencyMs(proxy)}
              onclick={() => onProxyClick(proxy)}
              ondblclick={() => onEditProxy(proxy)}
              triggerProps={props}
            />
          {/snippet}
        </ContextMenu.Trigger>
        <ContextMenu.Content>
          <ContextMenu.Item
            disabled={!canSelectProxy || !proxy.enabled}
            onclick={() => onSelectProxy(proxy.tag)}
          >
            <HugeiconsIcon icon={CheckIcon} />
            Select
          </ContextMenu.Item>
          <ContextMenu.Item onclick={() => onEditProxy(proxy)}>
            <HugeiconsIcon icon={Edit01Icon} />
            Edit
          </ContextMenu.Item>
          <ContextMenu.Item onclick={() => onTestProxy(proxy)}>
            <HugeiconsIcon icon={ZapIcon} />
            Test Proxy Latency
          </ContextMenu.Item>
          <ContextMenu.Item
            onclick={() => copyShareLink(proxy)}
          >
            <HugeiconsIcon icon={Copy01Icon} />
            Copy
          </ContextMenu.Item>
          <ContextMenu.Item onclick={() => showQr(proxy)}>
            <HugeiconsIcon icon={QrCodeIcon} />
            QR Code
          </ContextMenu.Item>
          <ContextMenu.Separator />
          <ContextMenu.Item
            variant="destructive"
            onclick={() => onDeleteProxy(proxy)}
          >
            <HugeiconsIcon icon={Delete01Icon} />
            Delete
          </ContextMenu.Item>
        </ContextMenu.Content>
      </ContextMenu.Root>
    {/each}

    {#if proxies.length === 0}
      <div
        class="col-span-full flex flex-col items-center justify-center gap-2 rounded-xl border border-dashed border-border/50 py-12 text-center"
      >
        <p class="text-muted-foreground text-sm">No proxies in this group</p>
        <Button variant="outline" size="sm" onclick={onAddProxy}>
          <HugeiconsIcon icon={Add01Icon} />
          Add Proxy
        </Button>
      </div>
    {/if}
  </div>
</div>

<ProxyQrDialog bind:open={qrOpen} link={qrLink} title={qrTitle} />
