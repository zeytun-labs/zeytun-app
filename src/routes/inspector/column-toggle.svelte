<script lang="ts">
  import * as DropdownMenu from "$lib/components/ui/dropdown-menu";
  import { Button } from "$lib/components/ui/button";
  import { Cancel01Icon, Settings02Icon, FilterIcon } from "@hugeicons/core-free-icons";
  import { HugeiconsIcon } from "@hugeicons/svelte";
  import type { Table } from "@tanstack/table-core";
  import type { ConnDto } from "$lib/core/types";
  import { COLUMN_LABELS } from "./columns";

  let { table }: { table: Table<ConnDto> } = $props();

  const toggleable = $derived(
    table.getAllColumns().filter((c) => c.getCanHide()),
  );
</script>

<DropdownMenu.Root>
  <DropdownMenu.Trigger>
    {#snippet child({ props })}
      <Button variant="outline" {...props}>
        <HugeiconsIcon icon={FilterIcon} class="size-3.5" />
        Columns
      </Button>
    {/snippet}
  </DropdownMenu.Trigger>
  <DropdownMenu.Content align="end" class="w-40">
    {#each toggleable as column (column.id)}
      <DropdownMenu.CheckboxItem
        checked={column.getIsVisible()}
        onCheckedChange={(v) => column.toggleVisibility(!!v)}
      >
        {COLUMN_LABELS[column.id] ?? column.id}
      </DropdownMenu.CheckboxItem>
    {/each}
  </DropdownMenu.Content>
</DropdownMenu.Root>
