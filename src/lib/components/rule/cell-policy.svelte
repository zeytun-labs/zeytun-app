<script lang="ts">
  import { HugeiconsIcon } from "@hugeicons/svelte";
  import { Alert01Icon } from "@hugeicons/core-free-icons";
  import { cn } from "$lib/utils";

  // The rule's resolved outbound name, plus an orphaned flag: when the rule's
  // target proxy was deleted, the backend repoints it to Direct and marks it
  // orphaned (Option A). Surface that as a warning chip.
  let { name, orphaned = false }: { name: string; orphaned?: boolean } =
    $props();

  const isDirect = $derived(name.toLowerCase() === "direct");
  const isReject = $derived(name.toLowerCase() === "reject" || name.toLowerCase() === "block");
</script>

<span class="flex min-w-0 items-center gap-1.5">
  <span
    class={cn(
      "truncate font-medium",
      isDirect ? "text-lime-500 dark:text-lime-300" : "",
      isReject ? "text-destructive/80 dark:text-destructive" : ""
    )}
  >
    {name}
  </span>
  {#if orphaned}
    <span
      class="text-destructive bg-destructive/10 inline-flex shrink-0 items-center gap-1 rounded px-1.5 py-0.5 text-[10px] font-medium"
      title="Target proxy was removed — repointed to Direct. Edit this rule to pick a new target."
    >
      <HugeiconsIcon icon={Alert01Icon} class="size-3" />
      orphaned
    </span>
  {/if}
</span>
