<script lang="ts">
  import { notificationStore } from "$lib/stores/notifications.svelte";
  import { Button } from "$lib/components/ui/button/index.js";
  import { coreStart } from "$lib/core/api";
  import { toast } from "svelte-sonner";

  const sticky = $derived(notificationStore.stickyCritical);
  let restarting = $state(false);

  async function handleRestartCore() {
    restarting = true;
    try {
      await coreStart();
      notificationStore.dismissAllCritical();
      toast.success("Core restarted");
    } catch {
      toast.error("Failed to restart core");
    } finally {
      restarting = false;
    }
  }
</script>

{#if sticky.length > 0}
  <div
    class="bg-destructive text-destructive-foreground flex shrink-0 items-center gap-2 px-3.5 py-1.5 text-xs transition-all duration-200"
    role="alert"
  >
    <span class="font-bold uppercase tracking-wider text-[10px] bg-destructive-foreground/15 px-1.5 py-0.5 rounded shrink-0">
      Critical
    </span>
    <span class="min-w-0 flex-1 truncate font-medium" title={sticky[0].message || sticky[0].title}>
      {sticky[0].title}{sticky[0].message ? ` — ${sticky[0].message}` : ""}
    </span>
    {#if sticky.length > 1}
      <span class="opacity-80 shrink-0 text-[11px] font-mono">+{sticky.length - 1}</span>
    {/if}
    {#if sticky[0].code === "SERVICE_FATAL"}
      <Button
        size="sm"
        variant="secondary"
        class="h-6 shrink-0 px-2 text-[11px] font-medium"
        disabled={restarting}
        onclick={handleRestartCore}
      >
        {restarting ? "Restarting…" : "Restart Core"}
      </Button>
    {/if}
    <Button
      size="sm"
      variant="secondary"
      class="h-6 shrink-0 px-2 text-[11px] font-medium opacity-90 hover:opacity-100"
      onclick={() => notificationStore.dismissCritical(sticky[0].id)}
    >
      Dismiss
    </Button>
    {#if sticky.length > 1}
      <Button
        size="sm"
        variant="ghost"
        class="h-6 shrink-0 px-2 text-[11px] text-destructive-foreground hover:bg-destructive-foreground/10"
        onclick={() => notificationStore.dismissAllCritical()}
      >
        Dismiss All
      </Button>
    {/if}
  </div>
{/if}
