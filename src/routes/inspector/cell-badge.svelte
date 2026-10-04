<script lang="ts">
  import Badge from "$lib/components/ui/badge/badge.svelte";

  let { value }: { value: string } = $props();
</script>

<Badge variant="outline" class="uppercase text-muted-foreground font-mono">
  {value}
</Badge>
