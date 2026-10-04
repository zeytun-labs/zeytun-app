<script lang="ts">
  import { Badge } from "$lib/components/ui/badge/index.js";
  import type { Row } from "@tanstack/table-core";
  import type { Schema } from "./schemas.js";
  let { row }: { row: Row<Schema> } = $props();
</script>
<div class="w-44">
  <Badge variant="secondary" class="text-muted-foreground px-1.5">
    {row.original.kind}
  </Badge>
</div>