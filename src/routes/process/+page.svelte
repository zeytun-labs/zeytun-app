<script lang="ts">
    import { onMount } from "svelte";
    import {
        dashboardStore,
        type ProcessStat,
    } from "$lib/stores/dashboardStore.svelte";
    import { profileStore } from "$lib/stores/profile.svelte";
    import { coreCloseConnections } from "$lib/core/api";
    import { toast } from "svelte-sonner";

    import HeaderControls from "./components/header-controls.svelte";
    import ProcessTable from "./components/process-table.svelte";
    import ProcessDetailPanel from "./components/process-detail-panel.svelte";
    import AddRuleDialog from "./components/add-rule-dialog.svelte";
  import { errorMessage } from "$lib/errors";

    let ruleDialogOpen = $state(false);
    let ruleProcessName = $state("");
    let ruleProcessPath = $state("");

    // Keep last process visible while the detail column collapses.
    let visibleProcess = $state<ProcessStat | null>(null);
    const detailOpen = $derived(dashboardStore.selectedProcessName !== null);

    $effect(() => {
        const p = dashboardStore.selectedProcess;
        if (p) visibleProcess = p;
    });

    onMount(async () => {
        try {
            await profileStore.load();
        } catch (e) {
            console.error("profileStore.load() failed:", e);
            toast.error("Failed to load profile", { description: errorMessage(e) });
        }
    });

    function openAddRule(processName: string, processPath: string) {
        ruleProcessName = processName;
        ruleProcessPath = processPath;
        ruleDialogOpen = true;
    }

    async function handleKillConnections(process: ProcessStat) {
        if (!process.connectionIds || process.connectionIds.length === 0) {
            toast.error(`No active connections for process: ${process.name}`);
            return;
        }

        try {
            await coreCloseConnections(process.connectionIds);
            toast.success(`Sent kill command for ${process.connectionIds.length} connection(s) of ${process.name}`);
        } catch (err) {
            toast.error("Failed to close connections", { description: errorMessage(err) });
        }
    }
</script>

<div
    class="flex min-h-0 min-w-0 flex-1"
>
    <div class="flex min-h-0 min-w-0 flex-1 flex-col gap-4">
        <HeaderControls
            bind:sortBy={dashboardStore.sortBy}
            bind:searchQuery={dashboardStore.searchQuery}
        />

        <div>
            <ProcessTable
                bind:selectedProcessName={dashboardStore.selectedProcessName}
                onAddRule={openAddRule}
                onKillConnections={handleKillConnections}
            />
        </div>
    </div>

    <div
        class="min-h-0 shrink-0 transition-[width,margin] duration-300 ease-out {detailOpen
            ? 'ms-4 w-90'
            : 'pointer-events-none ms-0 w-0'}"
        aria-hidden={!detailOpen}
    >
        <!-- fixed inner width; outer clips during collapse so no page scroll thrash -->
        <div class="h-[calc(100vh-6.5rem)] w-90 min-w-90">
            {#if visibleProcess}
                <ProcessDetailPanel
                    process={visibleProcess}
                    onClose={() => (dashboardStore.selectedProcessName = null)}                   
                />
            {/if}
        </div>
    </div>
</div>

<AddRuleDialog
    bind:open={ruleDialogOpen}
    processName={ruleProcessName}
    processPath={ruleProcessPath}
    onSuccess={(msg) => toast.success(msg)}
    onError={(err) => toast.error(err)}
/>
