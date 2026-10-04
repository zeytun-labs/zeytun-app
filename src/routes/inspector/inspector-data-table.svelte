<script lang="ts">
  import * as Table from "$lib/components/ui/table/index.js";
  import * as Tabs from "$lib/components/ui/tabs";
  import * as ContextMenu from "$lib/components/ui/context-menu";
  import { Input } from "$lib/components/ui/input";
  import { Button } from "$lib/components/ui/button";
  import { ListXIcon, Search01Icon } from "@hugeicons/core-free-icons";
  import { HugeiconsIcon } from "@hugeicons/svelte";
  import {
    getCoreRowModel,
    type ColumnDef,
    type VisibilityState,
  } from "@tanstack/table-core";
  import { createSvelteTable, FlexRender } from "$lib/components/ui/data-table";
  import type { ConnDto } from "$lib/core/types";
  import { buildColumns } from "./columns";
  import { hostOf, type Tab } from "./lib";
  import ColumnToggle from "./column-toggle.svelte";
  import ScrollArea from "$lib/components/ui/scroll-area/scroll-area.svelte";
  import * as InputGroup from "$lib/components/ui/input-group";

  interface Props {
    rows: ConnDto[];
    tab: Tab;
    search?: string;
    onTabChange: (t: Tab) => void;
    onClear: () => void;
    onRowClick: (r: ConnDto) => void;
    onAddHostRule: (r: ConnDto) => void;
    onAddProcessRule: (r: ConnDto) => void;
  }

  let {
    rows,
    tab,
    search = $bindable(""),
    onTabChange,
    onClear,
    onRowClick,
    onAddHostRule,
    onAddProcessRule,
  }: Props = $props();

  const columns: ColumnDef<ConnDto>[] = buildColumns();
  let columnVisibility = $state<VisibilityState>({});

  const table = createSvelteTable({
    get data() {
      return rows;
    },
    columns,
    state: {
      get columnVisibility() {
        return columnVisibility;
      },
    },
    getRowId: (r) => r.id,
    getCoreRowModel: getCoreRowModel(),
    onColumnVisibilityChange: (updater) => {
      columnVisibility =
        typeof updater === "function" ? updater(columnVisibility) : updater;
    },
  });

  // --- Row virtualization -------------------------------------------------
  // The recent buffer can hold up to 1000 rows; rendering them all (each with a
  // ContextMenu + FlexRender cells) is what froze the page and pinned memory.
  // We render only the rows visible in the scroll viewport, plus a small
  // overscan, and pad the table with spacer rows so the scrollbar stays right.
  const ROW_H = 37; // px; keep in sync with the row height class below
  const OVERSCAN = 8;

  let scrollTop = $state(0);
  let viewportH = $state(0);

  const modelRows = $derived(table.getRowModel().rows);
  const total = $derived(modelRows.length);

  const startIndex = $derived(
    Math.max(0, Math.floor(scrollTop / ROW_H) - OVERSCAN),
  );
  const endIndex = $derived(
    Math.min(
      total,
      startIndex + Math.ceil((viewportH || 600) / ROW_H) + OVERSCAN * 2,
    ),
  );
  const visibleRows = $derived(modelRows.slice(startIndex, endIndex));
  const padTop = $derived(startIndex * ROW_H);
  const padBottom = $derived(Math.max(0, (total - endIndex) * ROW_H));

  function onScroll(e: Event) {
    const target = e.target as HTMLElement;

    if (target.getAttribute("data-slot") === "scroll-area-viewport") {
      scrollTop = target.scrollTop;
    }
  }
</script>

<div class="flex min-h-0 flex-1 flex-col">
  <!-- Content header -->
  <header
    class="flex shrink-0 items-center gap-2 border-b border-border/40 px-4 py-3"
  >
    <Tabs.Root value={tab} onValueChange={(v) => onTabChange(v as Tab)}>
      <Tabs.List variant="primary">
        <Tabs.Trigger value="recent">Recent</Tabs.Trigger>
        <Tabs.Trigger value="active">Active</Tabs.Trigger>
      </Tabs.List>
    </Tabs.Root>

    <div class="relative w-64 ms-auto">
      <InputGroup.Root>
        <InputGroup.Input
          placeholder="Filter by host, process, or port ..."
          bind:value={search}
        />
        <InputGroup.Addon>
          <HugeiconsIcon icon={Search01Icon} />
        </InputGroup.Addon>
      </InputGroup.Root>
    </div>

    <ColumnToggle {table} />
    <Button
      variant="outline"
      size="icon"
      onclick={onClear}
      aria-label="Clear recent"
    >
      <HugeiconsIcon icon={ListXIcon} />
    </Button>
  </header>

  <div class="flex min-h-0 flex-1" bind:clientHeight={viewportH}>
    <ScrollArea class="h-full w-full flex-1" onscrollcapture={onScroll}>
      <table class="w-full caption-bottom text-sm">
        <Table.Header class="bg-muted z-10 sticky top-0">
          {#each table.getHeaderGroups() as headerGroup (headerGroup.id)}
            <Table.Row>
              {#each headerGroup.headers as header (header.id)}
                <Table.Head
                  class="text-xs uppercase tracking-wide text-muted-foreground h-fit py-2"
                >
                  {#if !header.isPlaceholder}
                    <FlexRender
                      content={header.column.columnDef.header}
                      context={header.getContext()}
                    />
                  {/if}
                </Table.Head>
              {/each}
            </Table.Row>
          {/each}
        </Table.Header>
        <Table.Body>
          {#if total}
            {#if padTop > 0}
              <tr aria-hidden="true"
                ><td
                  colspan={columns.length}
                  style="height: {padTop}px; padding: 0; border: 0"
                ></td></tr
              >
            {/if}
            {#each visibleRows as row (row.id)}
              <ContextMenu.Root>
                <ContextMenu.Trigger>
                  {#snippet child({ props })}
                    <Table.Row
                      {...props}
                      class="h-9.25 cursor-pointer"
                      onclick={() => onRowClick(row.original)}
                    >
                      {#each row.getVisibleCells() as cell (cell.id)}
                        <Table.Cell class="py-1.5">
                          {#if cell.column.id === "url"}
                            <div
                              class="max-w-40 truncate"
                              title={String(cell.getValue())}
                            >
                              <FlexRender
                                content={cell.column.columnDef.cell}
                                context={cell.getContext()}
                              />
                            </div>
                          {:else if cell.column.id === "status"}
                            <div class="w-1" title={String(cell.getValue())}>
                              <FlexRender
                                content={cell.column.columnDef.cell}
                                context={cell.getContext()}
                              />
                            </div>
                          {:else}
                            <FlexRender
                              content={cell.column.columnDef.cell}
                              context={cell.getContext()}
                            />
                          {/if}
                        </Table.Cell>
                      {/each}
                    </Table.Row>
                  {/snippet}
                </ContextMenu.Trigger>
                <ContextMenu.Content class="w-56">
                  <ContextMenu.Item onclick={() => onAddHostRule(row.original)}>
                    Add rule for {hostOf(row.original)}
                  </ContextMenu.Item>
                  <ContextMenu.Item
                    onclick={() => onAddProcessRule(row.original)}
                  >
                    Add rule for {row.original.processName || "process"}
                  </ContextMenu.Item>
                </ContextMenu.Content>
              </ContextMenu.Root>
            {/each}
            {#if padBottom > 0}
              <tr aria-hidden="true"
                ><td
                  colspan={columns.length}
                  style="height: {padBottom}px; padding: 0; border: 0"
                ></td></tr
              >
            {/if}
          {:else}
            <Table.Row>
              <Table.Cell
                colspan={columns.length}
                class="h-24 text-center text-sm"
              >
                No connections.
              </Table.Cell>
            </Table.Row>
          {/if}
        </Table.Body>
      </table>
    </ScrollArea>
  </div>
</div>
