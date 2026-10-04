<script lang="ts">
  import { notificationStore } from "$lib/stores/notifications.svelte";
  import { Button } from "$lib/components/ui/button/index.js";

  const sticky = $derived(notificationStore.stickyCritical);
</script>

{#if sticky.length > 0}
  <div
    class="bg-destructive text-destructive-foreground flex shrink-0 items-center gap-2 px-3 py-1.5 text-xs"
    role="alert"
  >
    <span class="font-semibold shrink-0">Critical</span>
    <span class="min-w-0 flex-1 truncate" title={sticky[0].message || sticky[0].title}>
      {sticky[0].title}{sticky[0].message ? ` — ${sticky[0].message}` : ""}
    </span>
    {#if sticky.length > 1}
      <span class="opacity-80 shrink-0">+{sticky.length - 1}</span>
    {/if}
    <Button
      size="sm"
      variant="secondary"
      class="h-6 shrink-0 px-2 text-[11px]"
      onclick={() => notificationStore.dismissCritical(sticky[0].id)}
    >
      Dismiss
    </Button>
  </div>
{/if}
