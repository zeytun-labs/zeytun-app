<script lang="ts">
  import { cubicOut } from "svelte/easing";
  import { prefersReducedMotion } from "svelte/motion";
  import { slide } from "svelte/transition";
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
    ArrowDown01Icon,
    CheckIcon,
    Copy01Icon,
    Delete01Icon,
    Edit01Icon,
    QrCodeIcon,
    ZapIcon,
  } from "@hugeicons/core-free-icons";
  import { errorMessage } from "$lib/errors";

  interface Props {
    sectionId: string;
    proxies: Proxy[];
    allProxies: Proxy[];
    canSelectProxy: boolean;
    sectionTitle: string;
    sectionKind: "subscription" | "manual";
    onProxyClick: (proxy: Proxy) => void;
    onEditProxy: (proxy: Proxy) => void;
    onSelectProxy: (tag: string) => void;
    onTestProxy: (proxy: Proxy) => void;
    onDeleteProxy: (proxy: Proxy) => void;
    onAddProxy: () => void;
  }

  let {
    sectionId,
    proxies,
    allProxies,
    canSelectProxy,
    sectionTitle,
    sectionKind,
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
  let expanded = $state(true);

  const contentId = $derived(`proxy-section-${sectionId}`);
  const motionDuration = $derived(prefersReducedMotion.current ? 0 : 240);

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

<section class="flex flex-col gap-3 rounded-3xl border border-border/40 bg-card/20 p-4">
  <button
    type="button"
    class="group flex w-full items-center justify-between gap-3 rounded-xl text-left outline-none focus-visible:ring-2 focus-visible:ring-ring/60"
    aria-expanded={expanded}
    aria-controls={contentId}
    onclick={() => (expanded = !expanded)}
  >
    <span class="flex min-w-0 items-center gap-2.5">
      <span class="truncate text-sm font-medium">{sectionTitle}</span>
      <span class="shrink-0 rounded-full border border-border/50 px-2 py-0.5 text-[10px] uppercase tracking-wide text-muted-foreground">
        {sectionKind === "subscription" ? "Subscription" : "Manual"}
      </span>
    </span>
    <span class="flex shrink-0 items-center gap-2 text-xs tabular-nums text-muted-foreground">
      {proxies.length}
      <HugeiconsIcon
        icon={ArrowDown01Icon}
        class="size-4 transition-transform duration-200 motion-reduce:transition-none {expanded ? 'rotate-180' : ''}"
        aria-hidden="true"
      />
    </span>
  </button>

  {#if expanded}
    <div
      id={contentId}
      class="overflow-hidden"
      transition:slide={{ duration: motionDuration, easing: cubicOut }}
    >
      {#if proxies.length > 0}
        <div class="grid gap-3 grid-cols-2 lg:grid-cols-4">
          {#each proxies as proxy (proxy.tag)}
            <div>
              <ContextMenu.Root>
                <ContextMenu.Trigger>
                  {#snippet child({ props })}
                    <ProxyCard
                      {proxy}
                      {allProxies}
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
                  <ContextMenu.Item onclick={() => copyShareLink(proxy)}>
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
            </div>
          {/each}
        </div>
      {:else}
        <div class="flex items-center justify-between gap-3 rounded-2xl border border-dashed border-border/50 px-4 py-5">
          <span class="text-sm text-muted-foreground">
            {sectionKind === "subscription" ? "No proxies in this subscription yet." : "Add a proxy server to start routing traffic."}
          </span>
          {#if sectionKind === "manual"}
            <Button variant="outline" size="sm" class="shrink-0 gap-1.5" onclick={onAddProxy}>
              <HugeiconsIcon icon={Add01Icon} class="size-4" />
              Add Proxy
            </Button>
          {/if}
        </div>
      {/if}
    </div>
  {/if}
</section>

<ProxyQrDialog bind:open={qrOpen} link={qrLink} title={qrTitle} />
