<script lang="ts">
  import {
    notificationStore,
    SCOPE_SERVICE,
    SCOPE_CLASH_MODE,
    SCOPE_RULESET,
    SCOPE_SUBSCRIPTION,
    SCOPE_LABELS,
    type CoreNotification,
  } from "$lib/stores/notifications.svelte";
  import { Button } from "$lib/components/ui/button/index.js";
  import * as Popover from "$lib/components/ui/popover/index.js";
  import { ScrollArea } from "$lib/components/ui/scroll-area/index.js";
  import { HugeiconsIcon } from "@hugeicons/svelte";
  import {
    Notification03Icon,
    Copy01Icon,
    Cancel01Icon,
  } from "@hugeicons/core-free-icons";
  import { goto } from "$app/navigation";
  import { writeText } from "@tauri-apps/plugin-clipboard-manager";
  import { toast } from "svelte-sonner";

  const unread = $derived(notificationStore.unreadCount);
  const stickyCount = $derived(notificationStore.stickyCritical.length);
  const visible = $derived(notificationStore.visibleItems);

  function dotClass(n: CoreNotification): string {
    if (n.severity >= 3) return "bg-destructive shadow-[0_0_6px_var(--color-destructive)]";
    if (n.severity >= 2) return "bg-destructive";
    if (n.severity === 1) return "bg-amber-500";
    if (!n.read) return "bg-primary shadow-[0_0_6px_var(--color-primary)]";
    return "bg-muted-foreground/20";
  }

  function subtitle(n: CoreNotification): string {
    if (n.scope === SCOPE_CLASH_MODE) {
      const mode = n.attrs?.mode || n.message;
      return mode ? `Now using ${mode}` : "Mode changed";
    }
    if (n.scope === SCOPE_SERVICE) {
      if (n.code === "SERVICE_STARTED") return "Service is running";
      if (n.code === "SERVICE_FATAL") return n.message || n.attrs?.error || "Fatal error";
      if (n.code === "SERVICE_STOPPING") return "Service is stopping";
      if (n.code === "SERVICE_STARTING") return "Service is starting";
      return n.attrs?.status || n.message || "Service event";
    }
    if (n.scope === SCOPE_RULESET) {
      if (n.code === "RULESET_READY" || n.code === "RULESET_UPDATED") return "Ready";
      return n.message || n.attrs?.error || "Unknown error";
    }
    return n.message || n.attrs?.dest_host || n.attrs?.process_name || "Notification";
  }

  function formatTime(ts_ms: number): string {
    if (!ts_ms) return "";
    const diff = Math.max(0, Date.now() - ts_ms);
    if (diff < 60_000) return "just now";
    if (diff < 3600_000) return `${Math.floor(diff / 60_000)}m ago`;
    if (diff < 86400_000) return `${Math.floor(diff / 3600_000)}h ago`;
    return `${Math.floor(diff / 86400_000)}d ago`;
  }

  async function handleCopy(text: string, e: MouseEvent) {
    e.stopPropagation();
    try {
      await writeText(text);
      toast.success("Copied to clipboard");
    } catch {
      // ignore
    }
  }

  function handleDelete(id: string, e: MouseEvent) {
    e.stopPropagation();
    notificationStore.deleteItem(id);
  }

  function handleRowClick(n: CoreNotification) {
    notificationStore.markRead(n.id);
    if (n.scope === SCOPE_RULESET) {
      void goto("/rule?tab=rulesets");
    } else if (n.scope === SCOPE_SUBSCRIPTION) {
      void goto("/policy");
    }
  }
</script>

<Popover.Root
  onOpenChange={(open) => {
    if (open) notificationStore.markAllRead();
  }}
>
  <Popover.Trigger>
    {#snippet child({ props })}
      <Button
        {...props}
        variant="ghost"
        size="icon"
        class="relative h-8 w-8 text-muted-foreground hover:text-foreground"
        aria-label="Notifications"
      >
        <HugeiconsIcon icon={Notification03Icon} class="size-4" />
        {#if unread > 0 || stickyCount > 0}
          <span
            class="absolute -right-0.5 -top-0.5 flex h-4 min-w-4 items-center justify-center rounded-full bg-destructive px-1 text-[10px] font-medium text-destructive-foreground shadow-sm"
          >
            {stickyCount > 0 ? "!" : unread > 9 ? "9+" : unread}
          </span>
        {/if}
      </Button>
    {/snippet}
  </Popover.Trigger>
  <Popover.Content
    class="w-96 p-0 gap-0 overflow-hidden rounded-2xl bg-card/90 backdrop-blur-2xl border border-border/50 shadow-2xl ring-1 ring-white/5"
    align="end"
  >
    <!-- Header -->
    <div class="flex items-center justify-between border-b border-border/40 px-4 py-3 bg-muted/15">
      <div class="flex items-center gap-2">
        <span class="text-[13px] font-semibold tracking-tight text-foreground">Notifications</span>
        {#if visible.length > 0}
          <span class="rounded-full bg-muted/60 border border-border/30 px-2 py-0.5 text-[10px] font-medium text-muted-foreground">
            {visible.length}
          </span>
        {/if}
      </div>
      {#if visible.length > 0}
        <div class="flex items-center gap-1">
          <Button
            variant="ghost"
            size="sm"
            class="h-6 px-2 text-[11px] text-muted-foreground hover:text-foreground"
            onclick={() => notificationStore.markAllRead()}
          >
            Mark read
          </Button>
          <Button
            variant="ghost"
            size="sm"
            class="h-6 px-2 text-[11px] text-muted-foreground hover:text-foreground"
            onclick={() => notificationStore.clear()}
          >
            Clear
          </Button>
        </div>
      {/if}
    </div>

    <!-- Notification List -->
    <ScrollArea class="**:data-[slot=scroll-area-viewport]:max-h-84">
      {#if visible.length === 0}
        <div class="flex flex-col items-center justify-center py-12 px-4 text-center">
          <div class="size-10 rounded-2xl bg-muted/30 border border-border/30 flex items-center justify-center mb-2.5 text-muted-foreground/60 shadow-inner">
            <HugeiconsIcon icon={Notification03Icon} class="size-4" />
          </div>
          <p class="text-[13px] font-medium text-foreground">No notifications</p>
          <p class="text-[11.5px] text-muted-foreground/70 mt-0.5">You're all caught up</p>
        </div>
      {:else}
        {#each visible as n (n.id)}
          {@const sub = subtitle(n)}
          {@const copyPayload = sub ? `${n.title || n.code}: ${sub}` : n.title || n.code}
          <div
            role="button"
            tabindex="0"
            class="group relative flex w-full items-start gap-3 py-3 px-4 border-b border-border/30 last:border-0 text-left transition-colors hover:bg-muted/30 cursor-pointer {n.severity >= 3 && !n.dismissed
              ? 'bg-destructive/5'
              : !n.read
                ? 'bg-primary/[0.04]'
                : ''}"
            onclick={() => handleRowClick(n)}
            onkeydown={(e) => {
              if (e.key === "Enter" || e.key === " ") handleRowClick(n);
            }}
          >
            <!-- Status Dot -->
            <span
              class="mt-1.5 size-2 shrink-0 rounded-full transition-colors {dotClass(n)}"
            ></span>

            <!-- Content -->
            <div class="min-w-0 flex-1 pr-6">
              <div class="flex items-center gap-1.5 flex-wrap">
                {#if n.severity >= 3}
                  <span class="text-destructive text-[9px] font-bold uppercase tracking-wider bg-destructive/10 px-1 py-0.2 rounded">
                    Critical
                  </span>
                {/if}
                <span class="text-[12.5px] font-medium text-foreground tracking-tight truncate max-w-[170px]">
                  {n.title || n.code}
                </span>
                {#if SCOPE_LABELS[n.scope]}
                  <span class="rounded-full bg-muted/60 border border-border/30 px-1.5 py-0.2 text-[9.5px] font-normal text-muted-foreground shrink-0">
                    {SCOPE_LABELS[n.scope]}
                  </span>
                {/if}
              </div>

              <p class="text-muted-foreground text-[11.5px] leading-snug line-clamp-2 mt-0.5" title={sub}>
                {sub}
              </p>

              {#if n.scope === SCOPE_RULESET}
                <p class="text-[10.5px] font-medium text-primary hover:underline mt-1">
                  View in Rulesets →
                </p>
              {:else if n.scope === SCOPE_SUBSCRIPTION}
                <p class="text-[10.5px] font-medium text-primary hover:underline mt-1">
                  View Subscriptions →
                </p>
              {/if}
            </div>

            <!-- Timestamp (normal state) -->
            <div class="shrink-0 group-hover:opacity-0 transition-opacity">
              <span class="text-[11px] text-muted-foreground/60 tracking-tight">
                {formatTime(n.ts_ms)}
              </span>
            </div>

            <!-- Hover Actions Bar (Raycast floating action pill) -->
            <div
              class="absolute top-2.5 right-3.5 flex items-center gap-0.5 bg-card/95 backdrop-blur-md px-1 py-0.5 rounded-lg border border-border/50 shadow-sm opacity-0 group-hover:opacity-100 transition-all pointer-events-none group-hover:pointer-events-auto"
            >
              <Button
                variant="ghost"
                size="icon"
                class="size-6 text-muted-foreground hover:text-foreground rounded-md"
                title="Copy details"
                onclick={(e) => handleCopy(copyPayload, e)}
              >
                <HugeiconsIcon icon={Copy01Icon} class="size-3" />
              </Button>
              <Button
                variant="ghost"
                size="icon"
                class="size-6 text-muted-foreground hover:text-destructive rounded-md"
                title="Dismiss"
                onclick={(e) => handleDelete(n.id, e)}
              >
                <HugeiconsIcon icon={Cancel01Icon} class="size-3" />
              </Button>
            </div>
          </div>
        {/each}
      {/if}
    </ScrollArea>
  </Popover.Content>
</Popover.Root>
