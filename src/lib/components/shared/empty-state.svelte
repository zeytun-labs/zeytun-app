<script lang="ts">
  import type { Snippet } from "svelte";

  interface Props {
    message: string;
    actions?: Snippet;
  }

  let { message, actions }: Props = $props();
</script>

<div
  class="text-muted-foreground flex flex-1 items-center justify-center py-12 text-sm"
>
  <div class="flex flex-col items-center gap-3 text-center">
    <p>{message}</p>
    {#if actions}
      {@render actions()}
    {/if}
  </div>
</div>
