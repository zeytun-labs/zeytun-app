<script lang="ts">
  import { HugeiconsIcon } from "@hugeicons/svelte";
  import { ClockFadingIcon } from "@hugeicons/core-free-icons";
  import { cn } from "$lib/utils";

  interface Props {
    expiresAt?: number;
    session?: boolean;
  }

  let { expiresAt, session = false }: Props = $props();

  function format(ms?: number) {
    if (!ms) return "—";
    try {
      return new Date(ms).toLocaleString();
    } catch {
      return String(ms);
    }
  }
</script>

{#if session}
  <span
    class="bg-primary/10 text-primary inline-flex shrink-0 items-center gap-1 rounded px-1.5 py-0.5 text-[10px] font-medium"
    title="Session rule — disappears after reloading the core/profile or changing policies."
  >
    <HugeiconsIcon icon={ClockFadingIcon} class="size-3" />
    Session
  </span>
{:else}
  <span class={cn(!expiresAt ? "text-muted-foreground" : "")}>
    {format(expiresAt)}
  </span>
{/if}
