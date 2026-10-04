<script lang="ts">
  import * as Table from "$lib/components/ui/table/index.js";
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
    onSelectRule: (id: number) => void;
    onEditRule: (id: number) => void;
    getActionName: (tag: string) => string;
    onReorder?: (rules: Schema[]) => void;
    onToggleEnabled?: (id: number, enabled: boolean) => void;
    disableReorder?: boolean;
    showExpires?: boolean;
  }

  let {
    rules,
    selectedRuleId,
    onSelectRule,
    onEditRule,
    getActionName,
    onReorder,
    onToggleEnabled,
    disableReorder = false,
    showExpires = false,
  }: Props = $props();

  function formatExpires(ms?: number) {
    if (!ms) return "—";
    try {
      return new Date(ms).toLocaleString();
    } catch {
      return String(ms);
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
    <div class="w-full text-sm">
      <div class="sticky top-0 uppercase text-xs bg-background z-20 grid {showExpires ? 'grid-cols-[32px_44px_192px_200px_160px_160px_1fr]' : 'grid-cols-[32px_44px_192px_200px_160px_1fr]'} px-2 border-b border-border/70">
        {#each table.getHeaderGroups() as headerGroup}
          {#each headerGroup.headers as header}
            <div
              class="h-8 flex items-center font-medium text-muted-foreground"
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
      </div>
      <div class="p-0">
        {#if table.getRowModel().rows?.length}
          {#each table.getRowModel().rows as row, idx (row.id)}
            {@render DraggableRow({ row })}
            {#if idx < table.getRowModel().rows.length - 1}
              <div class="h-px bg-border/40 mx-2" aria-hidden="true"></div>
            {/if}
          {/each}
        {:else}
          <div
            class="h-24 flex items-center justify-center text-muted-foreground"
          >
            No rules found.
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
    disabled: row.original.kind === "FINAL" || disableReorder,
  })}

  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    data-state={selectedRuleId === row.original.id ? "selected" : undefined}
    data-dragging={isDragging.current}
    class="drop-row relative h-11 grid {showExpires
      ? 'grid-cols-[32px_44px_192px_200px_160px_160px_1fr]'
      : 'grid-cols-[32px_44px_192px_200px_160px_1fr]'} items-center rounded-xl hover:bg-muted/30 data-[state=selected]:bg-muted/40 transition-colors z-0 cursor-pointer data-[dragging=true]:z-10 data-[dragging=true]:opacity-80 data-[dnd-dragging=true]:bg-background {row
      .original.enabled === false
      ? 'opacity-50'
      : ''}"
    onclick={() => onSelectRule(row.original.id)}
    ondblclick={() => onEditRule(row.original.id)}
    {@attach ref}
  >
    {#each row.getVisibleCells() as cell (cell.id)}
      <div
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
  </div>
{/snippet}

<style>
  /* Drop dnd-kit Feedback hack; grid doesn't need it. */
</style>
