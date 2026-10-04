<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import { listen, type UnlistenFn } from "@tauri-apps/api/event";
  import { profileStore } from "$lib/stores/profile.svelte";
  import { coreInspectorSnapshot, coreInspectorClear, coreInspectorSetActive } from "$lib/core/api";
  import type { ConnDto, InspectorSnapshot, RuleType } from "$lib/core/types";

  import ConnectionSidebar from "./connection-sidebar.svelte";
  import InspectorDataTable from "./inspector-data-table.svelte";
  import ConnectionDetailSheet from "./connection-detail-sheet.svelte";
  import AddRuleDialog from "./add-rule-dialog.svelte";
  import DashCard from "$lib/components/dashboard/dash-card.svelte";
  import { hostRuleFor, type GroupBy, type Tab } from "./lib";
  import * as Tooltip from "$lib/components/ui/tooltip";

  let snapshot = $state<InspectorSnapshot>({ active: [], recent: [] });
  let tab = $state<Tab>("recent");
  let groupBy = $state<GroupBy>("client");
  let search = $state("");
  let selectedGroup = $state<string | null>(null);
  let selected = $state<ConnDto | null>(null);

  let ruleDialog = $state<{
    open: boolean;
    kind: RuleType;
    value: string;
    label: string;
  }>({ open: false, kind: "DOMAIN", value: "", label: "" });

  // Recent = full history (live + closed ring buffer) in one chronological
  // stream, newest first. Active tab stays a pure live view.
  const rows = $derived(
    tab === "active"
      ? snapshot.active
      : [...snapshot.active, ...snapshot.recent].sort(
          (a, b) => b.createdAt - a.createdAt,
        ),
  );

  const filtered = $derived.by(() => {
    const q = search.trim().toLowerCase();
    return rows.filter((r) => {
      const key = groupBy === "client" ? r.processName : r.host;
      if (selectedGroup && key !== selectedGroup) return false;
      if (!q) return true;
      return (
        r.host.toLowerCase().includes(q) ||
        r.address.toLowerCase().includes(q) ||
        r.processName.toLowerCase().includes(q)
      );
    });
  });

  const policyName = (tag: string) => profileStore.getMemberName(tag);

  function openHostRule(r: ConnDto) {
    const { kind, value } = hostRuleFor(r);
    ruleDialog = { open: true, kind, value, label: "Host" };
  }

  function openProcessRule(r: ConnDto) {
    // Inspector = surgical: exact binary path via PROCESS-PATH (no regex needed).
    ruleDialog = {
      open: true,
      kind: "PROCESS-PATH",
      value: r.processPath,
      label: "Process",
    };
  }

  async function clearRecent() {
    await coreInspectorClear();
    snapshot = { ...snapshot, recent: [] };
  }

  let unlisten: UnlistenFn | null = null;

  onMount(async () => {
    void profileStore.load();
    // Tell the daemon this page is live so it pushes snapshots while we're here.
    void coreInspectorSetActive(true);
    // Initial pull — the daemon owns updates from here on via push events
    // (one `inspector-snapshot` per second at most, only when something
    // changed and only while this page is open).
    try {
      snapshot = await coreInspectorSnapshot();
    } catch {
      /* window may be tearing down */
    }
    unlisten = await listen<InspectorSnapshot>("inspector-snapshot", (e) => {
      snapshot = e.payload;
    });
  });

  onDestroy(() => {
    unlisten?.();
    void coreInspectorSetActive(false);
  });
</script>

<Tooltip.Provider>
  <div class="flex min-h-0 min-w-0 flex-1 gap-4">
    <DashCard class="h-[calc(100vh-6.5rem)] w-64 shrink-0">
      <ConnectionSidebar
        {rows}
        {groupBy}
        {selectedGroup}
        onGroupByChange={(g) => {
          groupBy = g;
          selectedGroup = null;
        }}
        onSelectGroup={(g) => (selectedGroup = g)}
      />
    </DashCard>

    <DashCard class="h-[calc(100vh-6.5rem)] min-w-0 flex-1">
      <InspectorDataTable
        rows={filtered}
        {tab}
        bind:search
        onTabChange={(t) => {
          tab = t;
          selectedGroup = null;
        }}
        onClear={clearRecent}
        onRowClick={(r) => (selected = r)}
        onAddHostRule={openHostRule}
        onAddProcessRule={openProcessRule}
      />
    </DashCard>
  </div>

  <ConnectionDetailSheet
    connection={selected}
    {policyName}
    onClose={() => (selected = null)}
    onAddHostRule={openHostRule}
    onAddProcessRule={openProcessRule}
  />

  <AddRuleDialog
    bind:open={ruleDialog.open}
    kind={ruleDialog.kind}
    value={ruleDialog.value}
    label={ruleDialog.label}
  />
</Tooltip.Provider>
