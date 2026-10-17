<script lang="ts">
  import type { Snippet } from "svelte";
  import { HugeiconsIcon } from "@hugeicons/svelte";
  import { Delete01Icon, Edit01Icon, MoreVerticalIcon } from "@hugeicons/core-free-icons";
  import * as ContextMenu from "$lib/components/ui/context-menu/index.js";
  import * as DropdownMenu from "$lib/components/ui/dropdown-menu/index.js";
  import { cn } from "$lib/utils";

  let {
    label,
    selected = false,
    disabled = false,
    class: className,
    onSelect,
    onEdit,
    onDelete,
    children,
  }: {
    label: string;
    selected?: boolean;
    disabled?: boolean;
    class?: string;
    onSelect: () => void;
    onEdit: () => void;
    onDelete: () => void;
    children: Snippet;
  } = $props();

  function isRowContent(event: MouseEvent) {
    return !(event.target instanceof Element && event.target.closest("button, input, select, a, [role='switch'], [data-dns-row-actions]"));
  }

  function edit() {
    if (!disabled) onEdit();
  }

  function remove() {
    if (!disabled) onDelete();
  }
</script>

<ContextMenu.Root>
  <ContextMenu.Trigger
    role="group"
    tabindex={disabled ? -1 : 0}
    aria-label={`${label}. Press Enter to edit, or open the actions menu.`}
    aria-disabled={disabled}
    data-state={selected ? "selected" : undefined}
    class={cn("w-full text-left focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring/50", className)}
    onclick={(event) => { if (!disabled && isRowContent(event)) onSelect(); }}
    ondblclick={(event) => { if (isRowContent(event)) edit(); }}
    onkeydown={(event) => {
      if (event.target === event.currentTarget && event.key === "Enter") {
        event.preventDefault();
        edit();
      }
    }}
  >
    {@render children()}
    <div data-dns-row-actions class="flex items-center justify-center">
      <DropdownMenu.Root>
        <DropdownMenu.Trigger
          {disabled}
          aria-label={`Actions for ${label}`}
          class="inline-flex size-8 shrink-0 items-center justify-center rounded-full text-muted-foreground hover:bg-muted/70 hover:text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring/50 disabled:pointer-events-none disabled:opacity-50"
        >
          <HugeiconsIcon icon={MoreVerticalIcon} class="size-4" />
        </DropdownMenu.Trigger>
        <DropdownMenu.Content align="end" class="w-40 max-h-[min(16rem,var(--bits-dropdown-menu-content-available-height,16rem))] overscroll-contain">
          <DropdownMenu.Item {disabled} onclick={edit}>
            <HugeiconsIcon icon={Edit01Icon} />Edit
          </DropdownMenu.Item>
          <DropdownMenu.Item {disabled} variant="destructive" onclick={remove}>
            <HugeiconsIcon icon={Delete01Icon} />Delete
          </DropdownMenu.Item>
        </DropdownMenu.Content>
      </DropdownMenu.Root>
    </div>
  </ContextMenu.Trigger>
  <ContextMenu.Content class="w-40 max-h-[min(16rem,var(--bits-context-menu-content-available-height,16rem))] overscroll-contain">
    <ContextMenu.Item {disabled} onclick={edit}>
      <HugeiconsIcon icon={Edit01Icon} />Edit
    </ContextMenu.Item>
    <ContextMenu.Item {disabled} variant="destructive" onclick={remove}>
      <HugeiconsIcon icon={Delete01Icon} />Delete
    </ContextMenu.Item>
  </ContextMenu.Content>
</ContextMenu.Root>
