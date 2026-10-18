<script lang="ts">
  import { mergeProps } from "bits-ui";
  import * as ContextMenu from "$lib/components/ui/context-menu/index.js";
  import * as DropdownMenu from "$lib/components/ui/dropdown-menu/index.js";
  import { Checkbox } from "$lib/components/ui/checkbox/index.js";
  import * as Empty from "$lib/components/ui/empty/index.js";
  import { Button } from "$lib/components/ui/button/index.js";
  import { HugeiconsIcon } from "@hugeicons/svelte";
  import { Add01Icon, Route01Icon, MoreVerticalIcon, Edit01Icon, Delete01Icon, PlayIcon, PauseIcon } from "@hugeicons/core-free-icons";
  import {
    getCoreRowModel,
    getFacetedRowModel,
    getFacetedUniqueValues,
    getFilteredRowModel,
    getSortedRowModel,
    type ColumnDef,
    type Row,
    type SortingState,
  } from "@tanstack/table-core";
  import type { Schema } from "./schemas";
  import {
    createSvelteTable,
    FlexRender,
    renderComponent,
  } from "../ui/data-table";
  import DataTableDragHandle from "./data-table-drag-handle.svelte";
  import DataTableType from "./data-table-type.svelte";
  import CellProcess, { PROCESS_KINDS } from "./cell-process.svelte";
  import CellPolicy from "./cell-policy.svelte";
  import CellEnabled from "./cell-enabled.svelte";
  import CellExpires from "./cell-expires.svelte";
  import { useSortable } from "@dnd-kit-svelte/svelte/sortable";
  import { DragDropProvider } from "@dnd-kit-svelte/svelte";
  import { move } from "@dnd-kit/helpers";
  import { RestrictToVerticalAxis } from "@dnd-kit/abstract/modifiers";
  // @ts-expect-error resolved via vite alias to the @dnd-kit-svelte's dom instance
  import { defaultPreset, Feedback } from "@dnd-kit/dom";

  // Disable dnd-kit's drop animation. It animates the drag clone back onto the
  // placeholder using DOM rects, but our reactive `rules` swap re-renders the
  // rows in the same frame, so the delta it computes is stale and the dropped
  // row visibly jumps before snapping into place. Every other default plugin
  // (sensors, autoscroll, sorting) is kept intact.
  const dndPlugins = [
    ...defaultPreset.plugins.filter((p: unknown) => p !== Feedback),
    Feedback.configure({ dropAnimation: null }),
  ];

  interface Props {
    rules: Schema[];
    selectedRuleId: number | null;
    selectedRuleIds: number[];
    onSelectionChange: (ids: number[]) => void;
    onDeleteRule: (id: number) => void;
    disabled?: boolean;
    onSelectRule: (id: number) => void;
    onEditRule: (id: number) => void;
    getActionName: (tag: string) => string;
    onReorder?: (rules: Schema[]) => void;
    onToggleEnabled?: (id: number, enabled: boolean) => void;
    disableReorder?: boolean;
    showExpires?: boolean;
    onAddRule?: () => void;
  }

  let {
    rules,
    selectedRuleId,
    selectedRuleIds,
    onSelectionChange,
    onDeleteRule,
    disabled = false,
    onSelectRule,
    onEditRule,
    getActionName,
    onReorder,
    onToggleEnabled,
    disableReorder = false,
    showExpires = false,
    onAddRule,
  }: Props = $props();

  let selectionAnchor: number | null = null;
  const selectableIds = $derived(rules.filter((r) => r.kind !== "FINAL").map((r) => r.id));
  const selectedVisibleCount = $derived(selectableIds.filter((id) => selectedRuleIds.includes(id)).length);
  const gridColumns = $derived(showExpires
    ? "grid-cols-[32px_32px_44px_192px_200px_160px_160px_minmax(0,1fr)_40px]"
    : "grid-cols-[32px_32px_44px_192px_200px_160px_minmax(0,1fr)_40px]");

  function toggleSelection(id: number, checked: boolean) {
    if (disabled) return;
    selectionAnchor = id;
    onSelectionChange(checked
      ? [...selectedRuleIds.filter((selected) => selected !== id), id]
      : selectedRuleIds.filter((selected) => selected !== id));
    if (checked) onSelectRule(id);
  }

  function selectRow(event: MouseEvent, rule: Schema) {
    if (disabled || (event.target instanceof Element && event.target.closest("button:not([data-slot='tooltip-trigger']), input, a, [role='switch'], [data-row-actions]"))) return;
    onSelectRule(rule.id);
    if (rule.kind === "FINAL") {
      onSelectionChange([]);
      return;
    }
    if (event.shiftKey && selectionAnchor !== null) {
      const ids = rows.map((row) => row.original).filter((r) => r.kind !== "FINAL").map((r) => r.id);
      const start = ids.indexOf(selectionAnchor);
      const end = ids.indexOf(rule.id);
      if (start !== -1 && end !== -1) {
        const range = ids.slice(Math.min(start, end), Math.max(start, end) + 1);
        onSelectionChange(event.metaKey || event.ctrlKey ? [...new Set([...selectedRuleIds, ...range])] : range);
        return;
      }
    }
    selectionAnchor = rule.id;
    if (event.metaKey || event.ctrlKey) {
      onSelectionChange(selectedRuleIds.includes(rule.id)
        ? selectedRuleIds.filter((id) => id !== rule.id)
        : [...selectedRuleIds, rule.id]);
    } else {
      onSelectionChange([rule.id]);
    }
  }

  const columns: ColumnDef<Schema>[] = $derived([
    {
      id: "drag",
      header: () => null,
      cell: ({ row }) =>
        row.original.kind === "FINAL"
          ? null
          : renderComponent(DataTableDragHandle, {
              disabled: disableReorder,
            }),
    },
    {
      id: "enabled",
      header: "",
      cell: ({ row }) =>
        row.original.kind === "FINAL"
          ? null
          : renderComponent(CellEnabled, {
              checked: row.original.enabled !== false,
              disabled,
              onChange: (v: boolean) => onToggleEnabled?.(row.original.id, v),
            }),
    },
    {
      accessorKey: "kind",
      id: "type",
      header: "Type",
      cell: ({ row }) => renderComponent(DataTableType, { row }),
    },
    {
      accessorKey: "value",
      id: "value",
      header: "Value",
      cell: ({ row }) =>
        PROCESS_KINDS.has(row.original.kind)
          ? renderComponent(CellProcess, {
              kind: row.original.kind,
              value: row.original.value,
            })
          : row.original.value || "*",
    },
    {
      accessorKey: "outbound",
      id: "policy",
      header: "Policy",
      cell: ({ row }) =>
        renderComponent(CellPolicy, {
          name: getActionName(row.original.outbound),
          orphaned: row.original.orphaned ?? false,
        }),
    },
    ...(showExpires
      ? [
          {
            accessorKey: "expires_at",
            id: "expires",
            header: "Expires",
            cell: ({ row }: { row: Row<Schema> }) =>
              renderComponent(CellExpires, {
                expiresAt: row.original.expires_at,
                session: row.original.session,
              }),
          } as ColumnDef<Schema>,
        ]
      : []),
    {
      accessorKey: "comment",
      id: "comment",
      header: "Comment",
      cell: ({ row }) => row.original.comment ? row.original.comment : "—",
    },
  ]);

  let sorting = $state<SortingState>([]);

  const table = createSvelteTable({
    get data() {
      return rules;
    },
    get columns() {
      return columns;
    },
    state: {
      get sorting() {
        return sorting;
      },
    },
    getRowId: (row) => row.id.toString(),
    enableRowSelection: true,
    autoResetPageIndex: false,
    getCoreRowModel: getCoreRowModel(),
    getSortedRowModel: getSortedRowModel(),
    getFacetedRowModel: getFacetedRowModel(),
    getFacetedUniqueValues: getFacetedUniqueValues(),
    getFilteredRowModel: getFilteredRowModel(),

    onSortingChange: (updater) => {
      if (typeof updater === "function") {
        sorting = updater(sorting);
      } else {
        sorting = updater;
      }
    },
  });
  // TanStack memoizes its row model outside Svelte. Track the input explicitly
  // so refreshes from another webview also invalidate the rendered rows.
  const rows = $derived.by(() => {
    rules;
    sorting;
    return table.getRowModel().rows;
  });

</script>

<div>
  <DragDropProvider
    plugins={dndPlugins}
    modifiers={[
      // @ts-expect-error @dnd-kit/abstract types are botched atm
      RestrictToVerticalAxis,
    ]}
    onBeforeDragStart={(e: any) => {
      // <tr position:fixed> leaves table layout → cells collapse to content.
      // Pin measured widths before Feedback lifts the row.
      const el = e?.operation?.source?.element as
        | HTMLTableRowElement
        | undefined;
      if (!el || el.tagName !== "TR") return;
      for (const cell of Array.from(el.cells)) {
        cell.style.width = `${cell.getBoundingClientRect().width}px`;
      }
    }}
    onDragEnd={(e: any) => {
      // Feedback restores its saved (already-pinned) widths after dragend — clear next frame.
      const el = e?.operation?.source?.element as
        | HTMLTableRowElement
        | undefined;
      requestAnimationFrame(() => {
        if (el?.tagName !== "TR") return;
        for (const cell of Array.from(el.cells)) {
          cell.style.width = "";
        }
      });
      if (e.canceled) return;
      const newRules = move(rules, e);
      if (onReorder) {
        onReorder(newRules);
      } else {
        rules = newRules;
      }
    }}
  >
    <!-- Use grid instead of table. This prevents table-layout issues on fixed dnd-kit feedback rows. -->
    <div role="grid" aria-label="Routing rules" aria-multiselectable="true" class="w-full text-sm">
      <div role="row" class="sticky top-0 uppercase text-xs bg-background z-20 grid {gridColumns} border-b border-border/70">
        <div role="columnheader" class="h-8 flex items-center justify-center">
          <Checkbox
            aria-label="Select all visible rules"
            disabled={disabled || selectableIds.length === 0}
            checked={selectableIds.length > 0 && selectedVisibleCount === selectableIds.length}
            indeterminate={selectedVisibleCount > 0 && selectedVisibleCount < selectableIds.length}
            onCheckedChange={(checked) => onSelectionChange(checked
              ? [...new Set([...selectedRuleIds, ...selectableIds])]
              : selectedRuleIds.filter((id) => !selectableIds.includes(id)))}
          />
        </div>
        {#each table.getHeaderGroups() as headerGroup (headerGroup.id)}
          {#each headerGroup.headers as header (header.id)}
            <div
              role="columnheader"
              class="h-8 flex items-center px-2 font-medium text-muted-foreground"
            >
              {#if !header.isPlaceholder}
                <FlexRender
                  content={header.column.columnDef.header}
                  context={header.getContext()}
                />
              {/if}
            </div>
          {/each}
        {/each}
        <div role="columnheader" class="h-8"><span class="sr-only">Actions</span></div>
      </div>
      <div role="rowgroup" class="p-0">
        {#if rows.length}
          {#each rows as row, idx (row.id)}
            {@render DraggableRow({ row })}
            {#if idx < rows.length - 1}
              <div class="h-px bg-border/40 mx-2" aria-hidden="true"></div>
            {/if}
          {/each}
        {:else}
          <div class="p-6">
            <Empty.Root class="border border-dashed border-border/50 bg-card/20 py-12">
              <Empty.Media variant="icon">
                <HugeiconsIcon icon={Route01Icon} class="size-5 text-muted-foreground" />
              </Empty.Media>
              <Empty.Header>
                <Empty.Title>No routing rules</Empty.Title>
                <Empty.Description>
                  Create rules to decide how traffic is routed based on application, domain, or IP.
                </Empty.Description>
              </Empty.Header>
              {#if onAddRule}
                <Empty.Content>
                  <Button variant="outline" size="sm" class="gap-1.5" onclick={onAddRule}>
                    <HugeiconsIcon icon={Add01Icon} class="size-4" />
                    Add Rule
                  </Button>
                </Empty.Content>
              {/if}
            </Empty.Root>
          </div>
        {/if}
      </div>
    </div>
  </DragDropProvider>
</div>

{#snippet DraggableRow({ row }: { row: Row<Schema> })}
  {@const { ref, isDragging, handleRef } = useSortable({
    id: row.original.id,
    index: () => row.index,
    disabled: row.original.kind === "FINAL" || disableReorder || disabled,
  })}

  <ContextMenu.Root onOpenChange={(open) => {
    if (!open || disabled) return;
    selectionAnchor = row.original.id;
    onSelectRule(row.original.id);
    onSelectionChange(row.original.kind === "FINAL" ? [] : [row.original.id]);
  }}>
    <ContextMenu.Trigger {disabled}>
      {#snippet child({ props })}
        <div
          {...mergeProps(props, {
            onkeydown: (event: KeyboardEvent) => {
              if (event.target !== event.currentTarget || disabled) return;
              if (event.key === "Enter") {
                event.preventDefault();
                onEditRule(row.original.id);
              } else if (event.key === " " && row.original.kind !== "FINAL") {
                event.preventDefault();
                toggleSelection(row.original.id, !selectedRuleIds.includes(row.original.id));
              }
            },
          })}
          role="row"
          aria-selected={selectedRuleIds.includes(row.original.id)}
          tabindex="0"
          aria-label={`Rule ${row.original.id}: ${row.original.kind} ${row.original.value}`}
          data-state={selectedRuleIds.includes(row.original.id) || (row.original.kind === "FINAL" && selectedRuleId === row.original.id) ? "selected" : undefined}
          data-dragging={isDragging.current}
          class="drop-row relative h-11 grid {gridColumns} items-center rounded-xl hover:bg-muted/30 data-[state=selected]:bg-muted/40 transition-colors z-0 cursor-pointer data-[dragging=true]:z-10 data-[dragging=true]:opacity-80 data-[dnd-dragging=true]:bg-background {row
            .original.enabled === false
            ? 'opacity-50'
            : ''}"
          onclick={(event) => selectRow(event, row.original)}
          ondblclick={(event) => {
            if (!disabled && !(event.target instanceof Element && event.target.closest("button:not([data-slot='tooltip-trigger']), input, a, [role='switch'], [data-row-actions]"))) onEditRule(row.original.id);
          }}
          {@attach ref}
        >
          <div role="gridcell" class="flex items-center justify-center">
            <Checkbox
              aria-label={`Select rule ${row.original.id}`}
              disabled={disabled || row.original.kind === "FINAL"}
              checked={selectedRuleIds.includes(row.original.id)}
              onclick={(event) => event.stopPropagation()}
              onCheckedChange={(checked) => toggleSelection(row.original.id, checked)}
            />
          </div>
          {#each row.getVisibleCells() as cell (cell.id)}
            <div
              role="gridcell"
              class="truncate px-2 {cell.column.id === 'value' ||
              cell.column.id === 'comment'
                ? 'overflow-hidden'
                : ''} {cell.column.id === 'comment'
                ? 'italic text-muted-foreground/60'
                : ''}"
            >
              <span
                class="block truncate"
                title={cell.column.id === "value" &&
                !PROCESS_KINDS.has(row.original.kind)
                  ? String(cell.getValue() ?? "")
                  : undefined}
              >
                <FlexRender
                  attach={handleRef}
                  content={cell.column.columnDef.cell}
                  context={cell.getContext()}
                />
              </span>
            </div>
          {/each}
          <div role="gridcell" data-row-actions class="flex items-center justify-center">
            <DropdownMenu.Root>
              <DropdownMenu.Trigger {disabled} aria-label={`Actions for rule ${row.original.id}`} class="inline-flex size-8 items-center justify-center rounded-md hover:bg-muted focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring">
                <HugeiconsIcon icon={MoreVerticalIcon} class="size-4" />
              </DropdownMenu.Trigger>
              <DropdownMenu.Content align="end" class="w-40">
                <DropdownMenu.Group>
                  <DropdownMenu.Item {disabled} onSelect={() => onEditRule(row.original.id)}>
                    <HugeiconsIcon icon={Edit01Icon} />Edit
                  </DropdownMenu.Item>
                  <DropdownMenu.Item
                    disabled={disabled || row.original.kind === "FINAL" || !onToggleEnabled}
                    onSelect={() => onToggleEnabled?.(row.original.id, row.original.enabled === false)}
                  >
                    <HugeiconsIcon icon={row.original.enabled === false ? PlayIcon : PauseIcon} />
                    {row.original.enabled === false ? "Enable" : "Disable"}
                  </DropdownMenu.Item>
                  <DropdownMenu.Item disabled={disabled || row.original.kind === "FINAL"} variant="destructive" onSelect={() => onDeleteRule(row.original.id)}>
                    <HugeiconsIcon icon={Delete01Icon} />Delete
                  </DropdownMenu.Item>
                </DropdownMenu.Group>
              </DropdownMenu.Content>
            </DropdownMenu.Root>
          </div>
        </div>
      {/snippet}
    </ContextMenu.Trigger>
    <ContextMenu.Content class="w-40">
      <ContextMenu.Group>
        <ContextMenu.Item {disabled} onSelect={() => onEditRule(row.original.id)}>
          <HugeiconsIcon icon={Edit01Icon} />Edit
        </ContextMenu.Item>
        <ContextMenu.Item
          disabled={disabled || row.original.kind === "FINAL" || !onToggleEnabled}
          onSelect={() => onToggleEnabled?.(row.original.id, row.original.enabled === false)}
        >
          <HugeiconsIcon icon={row.original.enabled === false ? PlayIcon : PauseIcon} />
          {row.original.enabled === false ? "Enable" : "Disable"}
        </ContextMenu.Item>
        <ContextMenu.Item disabled={disabled || row.original.kind === "FINAL"} variant="destructive" onSelect={() => onDeleteRule(row.original.id)}>
          <HugeiconsIcon icon={Delete01Icon} />Delete
        </ContextMenu.Item>
      </ContextMenu.Group>
    </ContextMenu.Content>
  </ContextMenu.Root>
{/snippet}

<style>
  /* Drop dnd-kit Feedback hack; grid doesn't need it. */
</style>
