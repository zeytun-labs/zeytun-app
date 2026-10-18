<script lang="ts">
  import { createActionQueue } from "$lib/composables/action-queue";
  import { useRowSelection } from "$lib/composables/row-selection.svelte";
  import SelectionRegion from "$lib/components/rule/selection-region.svelte";
  import RuleSelectionToolbar from "$lib/components/rule/rule-selection-toolbar.svelte";
  import { coreUpdateDnsRules } from "$lib/core/api";
  import { dnsRuleTargetExists } from "$lib/core/dns-servers";
  import type { DnsRuleDto, RuleSet } from "$lib/core/types";
  import { profileStore } from "$lib/stores/profile.svelte";
  import { useAsyncAction } from "$lib/composables/use-async-action.svelte";
  import { Skeleton } from "$lib/components/ui/skeleton/index.js";
  import DnsRuleTable, {
    type DnsRuleItem,
  } from "$lib/components/rule/dns-rule-table.svelte";
  import DnsRuleDialog from "$lib/components/rule/dns-rule-dialog.svelte";

  let {
    triggerAdd = $bindable(),
    search = $bindable(""),
  } = $props<{
    triggerAdd?: () => void;
    search?: string;
  }>();

  let selectedId = $state<string | null>(null);
  let dialogOpen = $state(false);
  let dialogMode = $state<"create" | "edit">("create");
  let editingRule = $state<DnsRuleItem | null>(null);

  const action = useAsyncAction();
  const writes = createActionQueue(() => profileStore.profile?.id);
  const dnsServers = $derived(profileStore.profile?.dns?.servers ?? []);
  const enabledRuleSets = $derived(
    ((profileStore.profile?.rule_sets ?? []) as RuleSet[]).filter(
      (ruleSet) => ruleSet.enabled,
    ),
  );
  const dnsRules = $derived(
    (profileStore.profile?.dns?.rules ?? []).map(
      (rule): DnsRuleItem => ({
        id: rule.id,
        kind: rule.kind,
        value: rule.value,
        target: rule.target,
        comment: rule.comment ?? undefined,
        enabled: rule.enabled,
      }),
    ),
  );
  const selection = useRowSelection(() => dnsRules.map((r) => r.id), () => profileStore.profile?.id);
  const selectedRules = $derived(dnsRules.filter((r) => selection.ids.includes(r.id)));
  const filteredRules = $derived.by(() => {
    const query = search.trim().toLowerCase();
    if (!query) return dnsRules;

    return dnsRules.filter((rule) =>
      [rule.kind, rule.value, rule.target, rule.comment ?? ""]
        .join(" ")
        .toLowerCase()
        .includes(query),
    );
  });

  $effect(() => {
    triggerAdd = openCreateDialog;
  });

  $effect(() => {
    if (dnsRules.length === 0) {
      selectedId = null;
      return;
    }
    if (!selectedId || !dnsRules.some((rule) => rule.id === selectedId)) {
      selectedId = dnsRules[0].id;
    }
  });

  async function persistRules(next: DnsRuleDto[]) {
    const profile = profileStore.profile;
    if (!profile) throw new Error("No active profile is available.");

    await coreUpdateDnsRules(next);
    await profileStore.refresh();
  }

  function openCreateDialog() {
    if (action.saving || profileStore.loading) return;
    dialogMode = "create";
    editingRule = null;
    dialogOpen = true;
  }

  function openEditDialog(id: string) {
    if (action.saving || profileStore.loading) return;
    const rule = dnsRules.find((item) => item.id === id);
    if (!rule) return;

    selectedId = id;
    dialogMode = "edit";
    editingRule = rule;
    dialogOpen = true;
  }

  async function saveRule(data: Omit<DnsRuleItem, "id" | "enabled">) {
    await writes.run(() => saveRuleDraft(data));
  }

  async function saveRuleDraft(data: Omit<DnsRuleItem, "id" | "enabled">) {
    const targetExists = dnsRuleTargetExists(data.target, dnsServers);
    if (!targetExists) {
      throw new Error("Select an available DNS server, Local (System), or Block.");
    }

    if (
      data.kind === "ruleset" &&
      !enabledRuleSets.some((ruleSet) => ruleSet.tag === data.value)
    ) {
      throw new Error("Select an enabled ruleset.");
    }

    if (dialogMode === "create") {
      const newRule: DnsRuleDto = {
        id: crypto.randomUUID(),
        kind: data.kind,
        value: data.value,
        target: data.target,
        comment: data.comment ?? null,
        enabled: true,
      };
      await persistRules([newRule, ...(profileStore.profile?.dns?.rules ?? [])]);
      selectedId = newRule.id;
      return;
    }

    if (!editingRule) {
      throw new Error("The DNS rule is no longer available.");
    }

    const next = (profileStore.profile?.dns?.rules ?? []).map((rule) =>
      rule.id === editingRule!.id
        ? {
            ...rule,
            kind: data.kind,
            value: data.value,
            target: data.target,
            comment: data.comment ?? null,
          }
        : rule,
    );
    await persistRules(next);
  }

  async function deleteRules(ids: string[]) {
    if (action.saving || profileStore.loading) return;
    await action.run(() => writes.run(async () => {
      await persistRules((profileStore.profile?.dns?.rules ?? []).filter((r) => !ids.includes(r.id)));
      selection.replace(selection.ids.filter((id) => !ids.includes(id)));
    }));
  }

  async function deleteRule(id: string) { await deleteRules([id]); }

  async function setEnabled(ids: string[], enabled: boolean) {
    if (action.saving) return;
    await action.run(() => writes.run(() => persistRules(
      (profileStore.profile?.dns?.rules ?? []).map((r) => ids.includes(r.id) ? { ...r, enabled } : r),
    )), undefined, { quiet: true });
  }

  async function toggleEnabled(id: string, enabled: boolean) { await setEnabled([id], enabled); }

  /** Final is the fallback resolver; specific rules and Hosts keep priority. */
  async function changeFinal(target: string | null) {
    if (action.saving) return;
    await action.run(() => writes.run(async () => {
      const profile = profileStore.profile;
      if (!profile) throw new Error("No active profile is available.");
      await profileStore.updateProfile({
        id: profile.id,
        dns: { ...profile.dns, final_server: target },
      });
    }));
  }
</script>

<div class="flex min-w-0 flex-col gap-3">

  <div class="min-w-0 overflow-x-auto">
    {#if profileStore.loading}
      <div class="flex min-w-170 flex-col gap-3 p-4">
        {#each [1, 2, 3, 4] as row (row)}
          <Skeleton class="h-11 w-full" />
        {/each}
      </div>
    {:else}
      <div class="min-w-170">
        <DnsRuleTable
          rules={filteredRules}
          dnsServers={profileStore.profile?.dns?.servers ?? []}
          selectedId={selectedId}
          {selection}
          onSelect={(id) => (selectedId = id)}
          onEdit={openEditDialog}
          onDelete={deleteRule}
          onToggleEnabled={toggleEnabled}
          finalTarget={profileStore.profile?.dns?.final_server ?? null}
          fakeIp={profileStore.profile?.dns?.fake_ip ?? false}
          outboundMode={profileStore.outboundMode}
          savedPolicy={profileStore.pendingRestart || !profileStore.isRunning}
          onFinalChange={changeFinal}
          disabled={action.saving}
        />
      </div>
    {/if}
  </div>
  <SelectionRegion open={selectedRules.length > 0}>
    <RuleSelectionToolbar selectedCount={selectedRules.length} itemLabel="DNS rule"
      canEnable={selectedRules.some((r) => r.enabled === false)}
      canDisable={selectedRules.some((r) => r.enabled !== false)} saving={action.saving}
      onEnable={() => setEnabled(selection.ids, true)} onDisable={() => setEnabled(selection.ids, false)}
      onDelete={() => deleteRules(selection.ids)} />
  </SelectionRegion>
</div>

<DnsRuleDialog
  bind:open={dialogOpen}
  mode={dialogMode}
  rule={editingRule}
  {dnsServers}
  ruleSets={enabledRuleSets}
  onSave={saveRule}
/>
