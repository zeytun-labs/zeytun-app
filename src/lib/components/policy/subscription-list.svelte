<script lang="ts">
  import { Button } from "$lib/components/ui/button";
  import * as AlertDialog from "$lib/components/ui/alert-dialog";
  import * as ContextMenu from "$lib/components/ui/context-menu/index.js";
  import * as RadioGroup from "$lib/components/ui/radio-group/index.js";
  import { Label } from "$lib/components/ui/label/index.js";
  import Spinner from "$lib/components/ui/spinner/spinner.svelte";
  import ProxyQrDialog from "./proxy-qr-dialog.svelte";
  import { writeText } from "@tauri-apps/plugin-clipboard-manager";
  import { subscriptionsStore } from "$lib/stores/subscriptions.svelte";
  import { profileStore } from "$lib/stores/profile.svelte";
  import type {
    SubscriptionSource,
    SubscriptionDeletePreview,
    SubscriptionDeleteMode,
  } from "$lib/core/types";
  import * as Empty from "$lib/components/ui/empty/index.js";
  import {
    Add01Icon,
    ReloadIcon,
    Edit01Icon,
    Delete01Icon,
    Link04Icon,
    AlertCircleIcon,
    Copy01Icon,
    QrCodeIcon,
  } from "@hugeicons/core-free-icons";
  import { HugeiconsIcon } from "@hugeicons/svelte";
  import { toast } from "svelte-sonner";
  import Badge from "../ui/badge/badge.svelte";

  interface Props {
    onEdit: (source: SubscriptionSource) => void;
    onAdd?: () => void;
  }

  let { onEdit, onAdd }: Props = $props();

  const sources = $derived(subscriptionsStore.sources);
  const loading = $derived(subscriptionsStore.loading);

  // QR Dialog state
  let qrOpen = $state(false);
  let qrLink = $state("");
  let qrTitle = $state("");

  function showQr(source: SubscriptionSource) {
    qrLink = source.settings.url;
    qrTitle = source.settings.name;
    qrOpen = true;
  }

  async function copySubscriptionUrl(source: SubscriptionSource) {
    if (!source.settings.url) return;
    try {
      await writeText(source.settings.url);
      toast.success("Subscription URL copied to clipboard.");
    } catch {
      toast.error("Failed to copy URL");
    }
  }

  // Delete dialog state
  let deleteDialogOpen = $state(false);
  let deletingSource = $state<SubscriptionSource | null>(null);
  let deletePreview = $state<SubscriptionDeletePreview | null>(null);
  let loadingPreview = $state(false);
  let deleteMode = $state<SubscriptionDeleteMode>("keep_as_manual");
  let deleting = $state(false);

  async function handleRefreshSingle(source: SubscriptionSource) {
    if (subscriptionsStore.isRefreshing(source.id)) return;
    try {
      const res = await subscriptionsStore.refresh(source.id);
      if (res.status === "accepted") {
        toast.success(`Synced "${source.settings.name}"`);
      } else {
        toast.error(`Sync failed for "${source.settings.name}"`);
      }
    } catch (e: any) {
      toast.error(`Failed to refresh: ${e?.message ?? e}`);
    }
  }

  async function openDeleteDialog(source: SubscriptionSource) {
    deletingSource = source;
    deletePreview = null;
    loadingPreview = true;
    deleteDialogOpen = true;
    deleteMode = "keep_as_manual";

    try {
      deletePreview = await subscriptionsStore.deletePreview(source.id);
      if (deletePreview.dependencies.length === 0) {
        deleteMode = "delete_nodes";
      }
    } catch (e: any) {
      toast.error(`Failed to inspect dependencies: ${e?.message ?? e}`);
      deleteDialogOpen = false;
    } finally {
      loadingPreview = false;
    }
  }

  async function confirmDelete() {
    if (!deletingSource || !deletePreview) return;
    deleting = true;
    try {
      await subscriptionsStore.delete({
        profile_id: deletePreview.profile_id,
        source_id: deletePreview.source_id,
        preview_token: deletePreview.preview_token,
        confirmed: true,
        mode: deleteMode,
      });
      toast.success(`Deleted subscription "${deletingSource.settings.name}"`);
      deleteDialogOpen = false;
    } catch (e: any) {
      toast.error(`Failed to delete: ${e?.message ?? e}`);
    } finally {
      deleting = false;
    }
  }

  function formatBytes(bytes: number): string {
    if (bytes <= 0) return "0 B";
    const units = ["B", "KB", "MB", "GB", "TB"];
    const i = Math.min(
      Math.floor(Math.log(bytes) / Math.log(1024)),
      units.length - 1,
    );
    return `${(bytes / Math.pow(1024, i)).toFixed(1)} ${units[i]}`;
  }

  function getSourceHost(urlStr: string): string {
    try {
      const u = new URL(urlStr);
      return u.hostname || "SUBSCRIPTION";
    } catch {
      return "SUBSCRIPTION";
    }
  }

  function getNodeCount(sourceId: string): number {
    return profileStore.proxies.filter((p) => p.source_id === sourceId).length;
  }

  function getUsageStats(userinfo?: SubscriptionSource["userinfo"]) {
    if (!userinfo) {
      return {
        usedFormatted: "0 B",
        totalFormatted: "Unlimited",
        total: 0,
        percentage: null,
      };
    }
    const used = (userinfo.upload ?? 0) + (userinfo.download ?? 0);
    const total = userinfo.total ?? 0;
    const percentage =
      total > 0 ? Math.min(100, Math.round((used / total) * 100)) : null;
    return {
      usedFormatted: formatBytes(used),
      totalFormatted: total > 0 ? formatBytes(total) : "Unlimited",
      total,
      percentage,
    };
  }

  function getExpirationInfo(expireUnixSec?: number | null) {
    if (!expireUnixSec || expireUnixSec <= 0) return null;
    const nowSec = Math.floor(Date.now() / 1000);
    const diffSec = expireUnixSec - nowSec;
    const daysLeft = Math.ceil(diffSec / 86400);
    const dateFormatted = new Date(expireUnixSec * 1000).toLocaleDateString(
      undefined,
      {
        month: "short",
        day: "numeric",
        year: "numeric",
      },
    );

    if (diffSec <= 0) {
      return {
        text: "Expired",
        dateFormatted,
        isExpired: true,
        isNear: false,
      };
    }
    if (daysLeft === 0) {
      return {
        text: "Expires today",
        dateFormatted,
        isExpired: false,
        isNear: true,
      };
    }
    if (daysLeft === 1) {
      return {
        text: "1 day left",
        dateFormatted,
        isExpired: false,
        isNear: true,
      };
    }
    return {
      text: `${daysLeft} days left`,
      dateFormatted,
      isExpired: false,
      isNear: daysLeft <= 7,
    };
  }
</script>

<div class="flex flex-col gap-4">
  {#if loading && sources.length === 0}
    <div class="flex items-center justify-center py-12">
      <Spinner class="size-6 text-muted-foreground" />
    </div>
  {:else if sources.length === 0}
    <Empty.Root class="border border-dashed border-border/50 bg-card/20 py-16">
      <Empty.Media variant="icon">
        <HugeiconsIcon icon={Link04Icon} class="size-5 text-muted-foreground" />
      </Empty.Media>
      <Empty.Header>
        <Empty.Title>No subscriptions yet</Empty.Title>
        <Empty.Description>
          Add a subscription URL to import and synchronize proxy nodes automatically.
        </Empty.Description>
      </Empty.Header>
      {#if onAdd}
        <Empty.Content>
          <Button variant="outline" size="sm" class="gap-1.5" onclick={onAdd}>
            <HugeiconsIcon icon={Add01Icon} class="size-4" />
            Add Subscription
          </Button>
        </Empty.Content>
      {/if}
    </Empty.Root>
  {:else}
    <div class="grid grid-cols-1 gap-3.5 sm:grid-cols-2 lg:grid-cols-3">
      {#each sources as source (source.id)}
        {@const isRefreshing = subscriptionsStore.isRefreshing(source.id)}
        {@const nodeCount = getNodeCount(source.id)}
        {@const usage = getUsageStats(source.userinfo)}
        {@const exp = getExpirationInfo(source.userinfo?.expire)}
        <ContextMenu.Root>
          <ContextMenu.Trigger>
            {#snippet child({ props })}
              <div
                {...props}
                role="button"
                tabindex="0"
                ondblclick={() => onEdit(source)}
                class="group bg-card/70 hover:bg-card border border-border/40 hover:border-border flex flex-col justify-between rounded-4xl p-4 pb-5! text-left transition-all gap-4 select-none cursor-default"
              >
                <!-- Top: Icon + Name + Host/Subtitle -->
                <div class="flex items-start justify-between gap-3">
                  <div class="flex items-center gap-3 min-w-0 flex-1">
                    <div
                      class="flex size-10 shrink-0 items-center justify-center rounded-xl bg-muted transition-colors group-hover:text-primary font-mono text-sm font-semibold text-foreground"
                    >
                      {nodeCount}
                    </div>
                    <div class="min-w-0 flex flex-col flex-1">
                      <div
                        class="truncate text-sm font-semibold w-full"
                        title={source.settings.name}
                      >
                        {source.settings.name}
                      </div>
                      <div
                        class="text-muted-foreground text-xs font-mono flex items-center gap-1.5 uppercase"
                      >
                        <span class="truncate"
                          >{getSourceHost(source.settings.url)}</span
                        >

                        {#if source.last_error}
                          <Badge variant="destructive" class="uppercase"
                            >Error</Badge
                          >
                        {/if}
                      </div>
                    </div>

                    <div>
                      <Button
                        size="icon"
                        variant="outline"
                        class="bg-card/70!"
                        disabled={isRefreshing}
                        onclick={(e) => {
                          e.stopPropagation();
                          handleRefreshSingle(source);
                        }}
                        title="Sync now"
                      >
                        {#if isRefreshing}
                          <Spinner class="size-4" />
                        {:else}
                          <HugeiconsIcon icon={ReloadIcon} />
                        {/if}
                        <span class="sr-only">Sync now</span>
                      </Button>
                    </div>
                  </div>
                </div>

                <!-- Middle: Usage Progress Bar (if userinfo present) -->
                {#if usage}
                  <div class="flex flex-col gap-2 rounded-2xl bg-muted/40 p-3">
                    <div class="flex items-center justify-between text-xs">
                      <span class="text-muted-foreground text-xs">Usage</span>
                      <span class="text-foreground text-xs">
                        {#if exp}
                          <span
                            class="text-xs truncate {exp.isExpired
                              ? 'text-destructive'
                              : exp.isNear
                                ? 'text-amber-500'
                                : 'text-muted-foreground'}"
                            title={exp.dateFormatted}
                          >
                            {exp.text}
                          </span>
                        {:else}
                          <span class="text-xs text-muted-foreground">
                            No Expiry
                          </span>
                        {/if}
                      </span>
                    </div>
                    <div
                      class="relative h-1.5 w-full overflow-hidden rounded-full bg-muted"
                    >
                      {#if usage.total > 0 && usage.percentage !== null}
                        <div
                          class="h-full rounded-full transition-all duration-300 {usage.percentage >
                          90
                            ? 'bg-destructive'
                            : usage.percentage > 75
                              ? 'bg-amber-500'
                              : 'bg-primary'}"
                          style="width: {usage.percentage}%"
                        ></div>
                      {:else}
                        <div
                          class="h-full w-full rounded-full"
                          style="background: repeating-linear-gradient(-45deg, color-mix(in oklab, var(--primary) 45%, transparent) 0 4px, color-mix(in oklab, var(--primary) 12%, transparent) 4px 8px)"
                        ></div>
                      {/if}
                    </div>

                    <div class="flex items-center justify-between text-xs">
                      <span class="text-muted-foreground text-xs"
                        >{usage.usedFormatted}</span
                      >
                      <span class="text-muted-foreground text-xs">
                        {#if usage.total > 0}
                          {usage.totalFormatted}
                        {:else}
                          <span class="text-muted-foreground ml-1 text-xs"
                            >Unlimited</span
                          >
                        {/if}
                        {#if usage.percentage !== null}
                          <span class="text-muted-foreground ml-1 text-xs"
                            >({usage.percentage}%)</span
                          >
                        {/if}</span
                      >
                    </div>
                  </div>
                {/if}

                <!-- Error notice if any -->
                {#if source.last_error}
                  <div
                    class="rounded-xl border border-destructive/20 bg-destructive/10 p-2 text-xs text-destructive flex items-center gap-1.5"
                    title={source.last_error}
                  >
                    <HugeiconsIcon
                      icon={AlertCircleIcon}
                      class="size-3.5 shrink-0"
                    />
                    <span class="truncate">{source.last_error}</span>
                  </div>
                {/if}
              </div>
            {/snippet}
          </ContextMenu.Trigger>
          <ContextMenu.Content>
            <ContextMenu.Item
              onclick={() => {
                setTimeout(() => void handleRefreshSingle(source), 50);
              }}
            >
              <HugeiconsIcon icon={ReloadIcon} />
              Sync Now
            </ContextMenu.Item>
            <ContextMenu.Item onclick={() => onEdit(source)}>
              <HugeiconsIcon icon={Edit01Icon} />
              Edit
            </ContextMenu.Item>
            <ContextMenu.Item onclick={() => copySubscriptionUrl(source)}>
              <HugeiconsIcon icon={Copy01Icon} />
              Copy URL
            </ContextMenu.Item>
            <ContextMenu.Item onclick={() => showQr(source)}>
              <HugeiconsIcon icon={QrCodeIcon} />
              QR Code
            </ContextMenu.Item>
            <ContextMenu.Separator />
            <ContextMenu.Item
              variant="destructive"
              onclick={() => openDeleteDialog(source)}
            >
              <HugeiconsIcon icon={Delete01Icon} />
              Delete
            </ContextMenu.Item>
          </ContextMenu.Content>
        </ContextMenu.Root>
      {/each}
    </div>
  {/if}
</div>

<!-- Delete Confirmation Dialog -->
<AlertDialog.Root bind:open={deleteDialogOpen}>
  <AlertDialog.Content class="sm:max-w-md">
    <AlertDialog.Header>
      <AlertDialog.Title>Delete Subscription</AlertDialog.Title>
      <AlertDialog.Description>
        {#if loadingPreview}
          Inspecting dependencies across policies, chains, and rules...
        {:else if deletePreview}
          Are you sure you want to delete "{deletingSource?.settings.name}"?
          {#if deletePreview.dependencies.length > 0}
            <div
              class="mt-3 rounded-xl border border-amber-500/30 bg-amber-500/10 p-3 text-xs text-amber-500"
            >
              <p class="font-medium">
                {deletePreview.dependencies.length} item(s) currently depend on proxies
                from this source:
              </p>
              <ul class="mt-1 list-disc pl-4 space-y-0.5 text-[11px]">
                {#each deletePreview.dependencies.slice(0, 5) as dep}
                  <li>{dep.kind}: {dep.owner} uses {dep.proxy_tag}</li>
                {/each}
                {#if deletePreview.dependencies.length > 5}
                  <li>...and {deletePreview.dependencies.length - 5} more</li>
                {/if}
              </ul>
            </div>
          {/if}
        {/if}
      </AlertDialog.Description>
    </AlertDialog.Header>

    {#if !loadingPreview && deletePreview}
      <RadioGroup.Root bind:value={deleteMode} class="flex flex-col gap-3 py-2">
        <div class="flex items-start gap-2.5">
          <RadioGroup.Item
            value="keep_as_manual"
            id="del-keep-manual"
            class="mt-0.5"
          />
          <div class="flex flex-col gap-0.5">
            <Label
              for="del-keep-manual"
              class="font-medium text-xs cursor-pointer"
            >
              Keep proxies as Manual (Recommended)
            </Label>
            <span class="text-[11px] text-muted-foreground">
              Converts existing proxies from this source to manual nodes so
              your policies and routing rules keep working.
            </span>
          </div>
        </div>

        <div class="flex items-start gap-2.5">
          <RadioGroup.Item
            value="delete_nodes"
            id="del-nodes"
            class="mt-0.5"
          />
          <div class="flex flex-col gap-0.5">
            <Label
              for="del-nodes"
              class="font-medium text-xs cursor-pointer"
            >
              Delete all nodes
            </Label>
            <span class="text-[11px] text-muted-foreground">
              Removes all proxies from this source. Dependent rules will
              fail-closed.
            </span>
          </div>
        </div>
      </RadioGroup.Root>
    {/if}

    <AlertDialog.Footer class="mt-4">
      <AlertDialog.Cancel disabled={deleting}>Cancel</AlertDialog.Cancel>
      <AlertDialog.Action
        onclick={confirmDelete}
        disabled={loadingPreview || deleting}
        class="bg-destructive text-destructive-foreground hover:bg-destructive/90"
      >
        {#if deleting}
          <Spinner class="mr-2 size-4 border-white/30 border-t-white" />
        {/if}
        Delete Subscription
      </AlertDialog.Action>
    </AlertDialog.Footer>
  </AlertDialog.Content>
</AlertDialog.Root>

<ProxyQrDialog bind:open={qrOpen} link={qrLink} title={qrTitle} />
