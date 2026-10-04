<script lang="ts">
  import ProcessIcon from "../process/components/process-icon.svelte";

  let { name, path }: { name: string; path: string } = $props();
</script>

<span class="flex min-w-0 items-center gap-1.5">
  <ProcessIcon {name} {path} class="size-6 shrink-0" />
  <span class="truncate" title={name}>{name || "System"}</span>
</span>
