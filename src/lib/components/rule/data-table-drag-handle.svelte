<script lang="ts">
  import { Button } from "$lib/components/ui/button/index.js";
  import { DragDropVerticalIcon } from "@hugeicons/core-free-icons";
  import { HugeiconsIcon } from "@hugeicons/svelte";
  import type { Attachment } from "svelte/attachments";
  let { attach, disabled = false }: { attach?: Attachment; disabled?: boolean } = $props();
</script>

<Button
  {@attach attach}
  variant="ghost"
  size="icon"
  {disabled}
  class="text-muted-foreground size-7 hover:bg-transparent {disabled ? 'opacity-0' : 'cursor-grab active:cursor-grabbing'}"
>
  <HugeiconsIcon
    icon={DragDropVerticalIcon}
    class="text-muted-foreground size-3"
  />
  <span class="sr-only">Drag to reorder</span>
</Button>
