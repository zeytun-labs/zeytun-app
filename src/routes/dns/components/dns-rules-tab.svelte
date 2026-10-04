<script lang="ts">
  import { HugeiconsIcon } from "@hugeicons/svelte";
  import {
    Add01Icon,
    Delete01Icon,
    Search01Icon,
  } from "@hugeicons/core-free-icons";
  import { coreUpdateDnsFinal, coreUpdateDnsRules } from "$lib/core/api";
  import type { DnsRuleDto, RuleSet } from "$lib/core/types";
  import { profileStore } from "$lib/stores/profile.svelte";
  import { useAsyncAction } from "$lib/composables/use-async-action.svelte";
  import { Button } from "$lib/components/ui/button/index.js";
  import * as InputGroup from "$lib/components/ui/input-group";
  import { Skeleton } from "$lib/components/ui/skeleton/index.js";
  import DnsRuleTable, {
    type DnsRuleItem,
  } from "$lib/components/rule/dns-rule-table.svelte";
  import DnsRuleDialog from "$lib/components/rule/dns-rule-dialog.svelte";

    let {
    triggerAdd = $bindable(),
    triggerDelete = $bindable(),
    selectedId = $bindable(null),
    search = $bindable(""),
  } = $props<{
    triggerAdd?: () => void;
    triggerDelete?: () => void;
    selectedId?: string | null;
    search?: string;
  }>();

    let dialogOpen = $state(false);
  let dialogMode = $state<"create" | "edit">("create");
  let editingRule = $state<DnsRuleItem | null>(null);

  const action = useAsyncAction();
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
    triggerDelete = deleteSelected;
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
    const dns = profile.dns ?? {};
    dns.rules = next;
    profile.dns = dns;
    profileStore.pendingRestart = true;
  }

  function openCreateDialog() {
    dialogMode = "create";
    editingRule = null;
    dialogOpen = true;
  }

  function openEditDialog(id: string) {
    const rule = dnsRules.find((item) => item.id === id);
    if (!rule) return;

    selectedId = id;
    dialogMode = "edit";
    editingRule = rule;
    dialogOpen = true;
  }

  async function saveRule(data: Omit<DnsRuleItem, "id" | "enabled">) {
    const targetExists =
      data.target === "block" ||
      dnsServers.some((server) => server.tag === data.target);
    if (!targetExists) {
      throw new Error("Select an available DNS server or Block.");
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

  async function deleteSelected() {
    if (!selectedId || action.saving) return;
    const next = (profileStore.profile?.dns?.rules ?? []).filter(
      (rule) => rule.id !== selectedId,
    );

    await action.run(async () => {
      await persistRules(next);
      selectedId = next[0]?.id ?? null;
    });
  }

  async function toggleEnabled(id: string, enabled: boolean) {
    if (action.saving) return;
    const next = (profileStore.profile?.dns?.rules ?? []).map((rule) =>
      rule.id === id ? { ...rule, enabled } : rule,
    );
    await action.run(() => persistRules(next));
  }

  /** `dns.final`: null selects the built-in local (system) resolver. */
  async function changeFinal(target: string | null) {
    if (action.saving) return;
    await action.run(async () => {
      const profile = profileStore.profile;
      if (!profile) throw new Error("No active profile is available.");

      await coreUpdateDnsFinal(target);
      const dns = profile.dns ?? {};
      dns.final_server = target;
      profile.dns = dns;
      profileStore.pendingRestart = true;
    });
  }
</script>

<div class="flex min-w-0 flex-col gap-4">
  <div class="min-w-0 overflow-x-auto">
    {#if profileStore.loading}
      <div class="flex min-w-170 flex-col gap-3 p-4">
        {#each Array(4) as _}
          <Skeleton class="h-11 w-full" />
        {/each}
      </div>
    {:else}
      <div class="min-w-170">
        <DnsRuleTable
          rules={filteredRules}
          dnsServers={profileStore.profile?.dns?.servers ?? []}
          {selectedId}
          onSelect={(id) => (selectedId = id)}
          onEdit={openEditDialog}
          onToggleEnabled={toggleEnabled}
          finalTarget={profileStore.profile?.dns?.final_server ?? null}
          onFinalChange={changeFinal}
          disabled={action.saving}
        />
      </div>
    {/if}
  </div>
  </div>

<DnsRuleDialog
  bind:open={dialogOpen}
  mode={dialogMode}
  rule={editingRule}
  {dnsServers}
  ruleSets={enabledRuleSets}
  onSave={saveRule}
/>
