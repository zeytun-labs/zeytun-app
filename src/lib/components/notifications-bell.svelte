<script lang="ts">
  import {
    notificationStore,
    SCOPE_SERVICE,
    SCOPE_CLASH_MODE,
    SCOPE_RULESET,
    SCOPE_CONNECTION_ASK,
    type CoreNotification,
  } from "$lib/stores/notifications.svelte";
  import { Button } from "$lib/components/ui/button/index.js";
  import * as Popover from "$lib/components/ui/popover/index.js";
  import { ScrollArea } from "$lib/components/ui/scroll-area/index.js";
  import { HugeiconsIcon } from "@hugeicons/svelte";
  import { Notification03Icon } from "@hugeicons/core-free-icons";
  import { goto } from "$app/navigation";
  import { profileStore } from "$lib/stores/profile.svelte";
  import RulesetRetryDialog from "$lib/components/rule/ruleset-retry-dialog.svelte";

  const unread = $derived(notificationStore.unreadCount);
  const stickyCount = $derived(notificationStore.stickyCritical.length);
  const visible = $derived(notificationStore.visibleItems);
  const summary = $derived(notificationStore.rulesetSummary);

  let retryOpen = $state(false);

  const retryActionGroups = $derived.by(() => {
    const direct = [{ tag: "direct", name: "Direct" }];
    if (!profileStore.profile) return [{ options: direct }];
    const policies = profileStore.profile.policies
      .filter((p) => p.tag !== "root-policy" && p.tag !== "ROOT_POLICY")
      .map((p) => ({
        tag: p.tag,
        name: p.name || p.tag,
      }));
    const proxies = profileStore.profile.proxies
      .filter((p) => p.enabled)
      .map((p) => ({ tag: p.tag, name: p.title || p.tag }));
    return [
      { options: direct },
      { label: "Policies", options: policies },
      { label: "Proxies", options: proxies },
    ];
  });

  function openRules() {
    void goto("/rule");
  }

  function openRulesets() {
    void goto("/rule?tab=rulesets");
  }

  function openConnections() {
    void goto("/connection-ask");
  }

  function openHome() {
    void goto("/");
  }

  function dotClass(severity: number): string {
    if (severity >= 3) return "bg-destructive";
    if (severity >= 2) return "bg-destructive/80";
    if (severity === 1) return "bg-amber-500";
    return "bg-muted-foreground/40";
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
    // CONNECTION_ASK and others: use message directly (proc → dest)
    return n.message || n.attrs?.dest_host || n.attrs?.process_name || "Notification";
  }

  function canRetry(n: CoreNotification) {
    return (
      n.scope === SCOPE_RULESET &&
      (n.code === "RULESET_INITIAL_FETCH_FAILED" || n.code === "RULESET_UPDATE_FAILED")
    );
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
        class="relative h-8 w-8"
        aria-label="Notifications"
      >
        <HugeiconsIcon icon={Notification03Icon} />
        {#if unread > 0 || stickyCount > 0}
          <span
            class="absolute -right-0.5 -top-0.5 flex h-4 min-w-4 items-center justify-center rounded-full bg-destructive px-1 text-[10px] font-medium text-background"
          >
            {stickyCount > 0 ? "!" : unread > 9 ? "9+" : unread}
          </span>
        {/if}
      </Button>
    {/snippet}
  </Popover.Trigger>
  <Popover.Content class="w-80 p-0 gap-0 overflow-hidden" align="end">
    <div class="flex items-center justify-between border-b px-3 py-2">
      <span class="text-sm font-medium">Notifications</span>
      <Button
        variant="ghost"
        size="sm"
        class="h-7 text-xs"
        onclick={() => notificationStore.clear()}
      >
        Clear
      </Button>
    </div>

    <ScrollArea class="**:data-[slot=scroll-area-viewport]:max-h-72">
      {#if summary}
        <button
          type="button"
          class="w-full text-left py-2.5 px-4 border-b hover:bg-muted/50 {summary.severity >= 2
            ? 'bg-destructive/5'
            : ''}"
          onclick={summary.failedCount > 0 ? openRulesets : openRulesets}
        >
          <div class="flex items-start gap-2">
            <span
              class="mt-1 size-2 shrink-0 rounded-full {dotClass(summary.severity)}"
            ></span>
            <div class="min-w-0 flex-1">
              <p class="truncate text-sm font-medium">{summary.title}</p>
              <p class="text-muted-foreground truncate text-xs">{summary.subtitle}</p>
            </div>
          </div>
        </button>
      {/if}

        {#if visible.length === 0}
          {#if !summary}
            <p class="text-muted-foreground p-4 text-center text-sm">
              No notifications
            </p>
          {/if}
        {:else}
          {#each visible as n (n.id)}
            {@const retry = canRetry(n)}
            <button
              type="button"
              class="w-full text-left py-2.5 px-4 border-b last:border-0 hover:bg-muted/50 {n.severity >= 3 && !n.dismissed
                ? 'bg-destructive/5'
                : ''}"
              onclick={() => {
                if (retry) openRulesets();
                else if (n.scope === SCOPE_RULESET) openRulesets();
                else if (n.scope === SCOPE_CONNECTION_ASK) openConnections();
                else openHome();
              }}
            >
              <div class="flex items-start gap-2">
                <span
                  class="mt-1 size-2 shrink-0 rounded-full {dotClass(n.severity)}"
                ></span>
                <div class="min-w-0 flex-1">
                  <div class="flex items-center gap-1.5">
                    {#if n.severity >= 3}
                      <span class="text-destructive text-[10px] font-semibold uppercase">Critical</span>
                    {/if}
                    <p class="truncate text-sm font-medium">{n.title || n.code}</p>
                  </div>
                  <p class="text-muted-foreground truncate text-xs">
                    {subtitle(n)}
                  </p>
                </div>
              </div>
            </button>
          {/each}
        {/if}
      </ScrollArea>
  </Popover.Content>
</Popover.Root>

<RulesetRetryDialog
  bind:open={retryOpen}
  actionGroups={retryActionGroups}
  onSuccess={() => profileStore.refresh()}
/>
