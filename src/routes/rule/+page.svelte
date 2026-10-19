<script lang="ts">
  import { onMount } from "svelte";
  import {
    coreUpdateRules,
    coreUpdateRulesets,
    coreUpdateTempRules,
    corePruneExpiredTempRules,
    corePromoteTempRule,
  } from "$lib/core/api";
  import type { Rule, RuleSet, RuleType, TempRule } from "$lib/core/types";
  import type { RuleFormSchema } from "$lib/core/rule-schema";

  import { profileStore } from "$lib/stores/profile.svelte";
  import { notificationStore } from "$lib/stores/notifications.svelte";
  import { useAsyncAction } from "$lib/composables/use-async-action.svelte";

  import { createActionQueue } from "$lib/composables/action-queue";
  import { useRowSelection } from "$lib/composables/row-selection.svelte";
  import SelectionRegion from "$lib/components/rule/selection-region.svelte";
  import RuleSelectionToolbar from "$lib/components/rule/rule-selection-toolbar.svelte";
  import RuleToolbar from "$lib/components/rule/rule-toolbar.svelte";
  import RuleTable from "$lib/components/rule/rule-table.svelte";
  import RuleDialog, {
    type ActionGroup,
  } from "$lib/components/rule/rule-dialog.svelte";
  import RulesetDialog, {
    type RulesetDraftPayload,
  } from "$lib/components/rule/ruleset-dialog.svelte";
  import RulesetTable from "$lib/components/rule/ruleset-table.svelte";

  import { page } from "$app/state";

  import { Skeleton } from "$lib/components/ui/skeleton/index.js";
  import ScrollArea from "$lib/components/ui/scroll-area/scroll-area.svelte";
  import * as Tabs from "$lib/components/ui/tabs/index.js";
  import { HugeiconsIcon } from "@hugeicons/svelte";
  import { Search01Icon } from "@hugeicons/core-free-icons";
  import * as InputGroup from "$lib/components/ui/input-group";

  let search = $state("");
  let selectedRuleId = $state<number | null>(null);
  let selectedRuleIds = $state<number[]>([]);
  let selectedTempRuleIds = $state<number[]>([]);
  let selectedTempRuleId = $state<number | null>(null);
  let selectedRulesetId = $state<string | null>(null);

  let dialogOpen = $state(false);
  let dialogMode = $state<"create" | "edit">("create");
  let dialogTemp = $state(false);
  let editingRule = $state<Rule | TempRule | null>(null);

  let rulesetDialogOpen = $state(false);
  let rulesetDialogMode = $state<"create" | "edit">("create");
  let editingRuleset = $state<RuleSet | null>(null);

  let tab = $state<"rules" | "temp" | "rulesets">("rules");

  $effect(() => {
    const urlTab = page.url.searchParams.get("tab");
    if (urlTab === "rulesets" || urlTab === "temp" || urlTab === "rules") {
      tab = urlTab;
    }
  });

  const action = useAsyncAction();
  const writes = createActionQueue(() => profileStore.profile?.id);

  const rules = $derived((profileStore.profile?.rules ?? []) as Rule[]);
  const tempRules = $derived([
    ...(profileStore.profile?.temp_rules ?? []),
    ...(profileStore.profile?.session_rules ?? []),
  ] as TempRule[]);
  const ruleSets = $derived(
    (profileStore.profile?.rule_sets ?? []) as RuleSet[],
  );

  const actionGroups = $derived.by<ActionGroup[]>(() => {
    if (!profileStore.profile)
      return [
        {
          label: "Built-in",
          options: [
            { tag: "direct", name: "Direct" },
            { tag: "block", name: "Block" },
          ],
        },
      ];

    const builtin = [
      { tag: "direct", name: "Direct" },
      { tag: "block", name: "Block" },
    ];

    const policies = profileStore.profile.policies
      .filter((p) => p.tag !== "root-policy")
      .map((p) => ({ tag: p.tag, name: p.name || p.tag }));

    const proxies = profileStore.profile.proxies
      .filter((p) => p.enabled)
      .map((p) => ({ tag: p.tag, name: p.title || p.tag }));

    return [
      { options: builtin },
      { label: "Policies", options: policies },
      { label: "Proxies", options: proxies },
    ];
  });

  const rulesetsActionGroups = $derived.by<ActionGroup[]>(() => {
    const direct = [{ tag: "direct", name: "Direct" }];
    if (!profileStore.profile) return [{ options: direct }];
    const policies = profileStore.profile.policies
      .filter((p) => p.tag !== "root-policy" && p.tag !== "ROOT_POLICY")
      .map((p) => ({
        tag: p.tag,
        name: p.name || p.tag,
      }));
    const proxies = profileStore.profile.proxies
      .filter((p) => p.enabled)
      .map((p) => ({ tag: p.tag, name: p.title || p.tag }));
    return [
      { options: direct },
      { label: "Policies", options: policies },
      { label: "Proxies", options: proxies },
    ];
  });

  let globalDownloadPolicy = $state("direct");

  $effect(() => {
    if (profileStore.profile && globalDownloadPolicy === "direct") {
      const stored = localStorage.getItem(`rs_pol_${profileStore.profile.id}`);
      if (!stored) {
        const rs = ruleSets.find((r) => r.type === "remote");
        if (rs && rs.download_policy) {
          globalDownloadPolicy = rs.download_policy;
          localStorage.setItem(`rs_pol_${profileStore.profile.id}`, rs.download_policy);
        }
      } else {
        globalDownloadPolicy = stored;
      }
    }
  });

  async function updateGlobalDownloadPolicy(val: string) {
    globalDownloadPolicy = val;
    if (!profileStore.profile) return;
    localStorage.setItem(`rs_pol_${profileStore.profile.id}`, val);

    const updatedRuleSets = ruleSets.map(rs => {
      if (rs.type === "remote") {
         return { ...rs, download_policy: val };
      }
      return rs;
    });

    await action.run(async () => {
      await persistRuleSets(updatedRuleSets);
    }, "Update ruleset download policy");
  }

  const getActionName = (tag: string) => {
    if (tag === "root-policy") return "Global Proxy";
    for (const group of actionGroups) {
      const opt = group.options.find((o) => o.tag === tag);
      if (opt) return opt.name;
    }
    const p = profileStore.profile?.policies.find((x) => x.tag === tag);
    if (p) return p.name || p.tag;
    return tag;
  };

  const filteredRules = $derived.by(() => {
    const query = search.trim().toLowerCase();
    if (!query) return rules;
    return rules.filter((r) =>
      [r.id, r.kind, r.value, r.outbound, r.comment]
        .join(" ")
        .toLowerCase()
        .includes(query),
    );
  });

  const filteredTempRules = $derived.by(() => {
    const query = search.trim().toLowerCase();
    if (!query) return tempRules;
    return tempRules.filter((r) =>
      [r.id, r.kind, r.value, r.outbound, r.comment, r.expires_at]
        .join(" ")
        .toLowerCase()
        .includes(query),
    );
  });


  const selectedRules = $derived(rules.filter((r) => r.kind !== "FINAL" && selectedRuleIds.includes(r.id)));
  const selectedTempRules = $derived(tempRules.filter((r) => selectedTempRuleIds.includes(r.id)));
  const rulesetSelection = useRowSelection(() => ruleSets.map((r) => r.id), () => profileStore.profile?.id);
  const selectedRuleSets = $derived(ruleSets.filter((r) => rulesetSelection.ids.includes(r.id)));
  const selectedRows = $derived(tab === "temp" ? selectedTempRules : tab === "rules" ? selectedRules : selectedRuleSets);
  const canEnableSelected = $derived(selectedRows.some((r) => r.enabled === false));
  const canDisableSelected = $derived(selectedRows.some((r) => r.enabled !== false));

  const selectionScope = $derived(profileStore.profile?.id);
  $effect(() => {
    selectionScope;
    selectedRuleIds = [];
    selectedTempRuleIds = [];
  });

  // Drop selected IDs that no longer exist after an external refresh.
  $effect(() => {
    const ids = selectedRules.map((r) => r.id);
    if (ids.length !== selectedRuleIds.length) selectedRuleIds = ids;
  });
  $effect(() => {
    const ids = selectedTempRules.map((r) => r.id);
    if (ids.length !== selectedTempRuleIds.length) selectedTempRuleIds = ids;
  });

  $effect(() => {
    const sets = ruleSets;
    if (sets.length === 0) {
      selectedRulesetId = null;
      return;
    }
    if (!selectedRulesetId || !sets.some((r) => r.id === selectedRulesetId)) {
      selectedRulesetId = sets[0].id;
    }
  });

  $effect(() => {
    if (
      selectedRuleId !== null &&
      !rules.some((r) => r.id === selectedRuleId)
    ) {
      selectedRuleId = rules[0]?.id ?? null;
    }
  });

  $effect(() => {
    if (
      selectedTempRuleId !== null &&
      !tempRules.some((r) => r.id === selectedTempRuleId)
    ) {
      selectedTempRuleId = tempRules[0]?.id ?? null;
    }
  });


  const canPromote = $derived(tab === "temp" && selectedTempRules.length === 1);

  onMount(async () => {
    await refreshProfile();
    try {
      const pruned = await corePruneExpiredTempRules();
      if (pruned) await refreshProfile();
    } catch {
      /* core not up yet */
    }
  });

  async function refreshProfile() {
    await action.run(async () => {
      await profileStore.load();
      selectedRuleId =
        rules[0]?.id ?? profileStore.profile?.rules?.[0]?.id ?? null;
      selectedTempRuleId = tempRules[0]?.id ?? null;
    });
  }

  async function persistRules(next: Rule[]) {
    if (!profileStore.profile) return;
    await action.run(() => writes.run(async () => {
      await coreUpdateRules(next.map((r) => ({ ...r, enabled: rules.find((current) => current.id === r.id)?.enabled ?? r.enabled })));
      await profileStore.refresh();
    }));
  }

  async function persistTempRules(next: TempRule[]) {
    if (!profileStore.profile) return;
    await action.run(() => writes.run(async () => {
      await coreUpdateTempRules(next.map((r) => ({ ...r, enabled: tempRules.find((current) => current.id === r.id)?.enabled ?? r.enabled })));
      await profileStore.refresh();
    }));
  }

  async function persistRuleSets(next: RuleSet[]) {
    if (!profileStore.profile) throw new Error("No active profile is available.");
    await writes.run(async () => {
      await coreUpdateRulesets(next.map((r) => ({ ...r, enabled: ruleSets.find((current) => current.id === r.id)?.enabled ?? r.enabled })));
      await profileStore.refresh();
    });
  }

  function handleAddClick() {
    dialogTemp = false;
    dialogMode = "create";
    editingRule = null;
    dialogOpen = true;
  }

  function handleAddTempClick() {
    dialogTemp = true;
    dialogMode = "create";
    editingRule = null;
    dialogOpen = true;
    tab = "temp";
  }


  function handleAddRuleset() {
    rulesetDialogMode = "create";
    editingRuleset = null;
    rulesetDialogOpen = true;
  }

  function handleEditClick(ruleId: number) {
    const rule = rules.find((r) => r.id === ruleId);
    if (!rule) return;
    selectedRuleId = ruleId;
    selectedRuleIds = rule.kind === "FINAL" ? [] : [ruleId];
    dialogTemp = false;
    dialogMode = "edit";
    editingRule = rule;
    dialogOpen = true;
  }

  function handleEditTempClick(ruleId: number) {
    const rule = tempRules.find((r) => r.id === ruleId);
    if (!rule) return;
    selectedTempRuleId = ruleId;
    selectedTempRuleIds = [ruleId];
    dialogTemp = true;
    dialogMode = "edit";
    editingRule = rule;
    dialogOpen = true;
  }


  function handleEditRuleset(id: string) {
    const rs = ruleSets.find((r) => r.id === id);
    if (!rs) return;
    selectedRulesetId = id;
    rulesetDialogMode = "edit";
    editingRuleset = rs;
    rulesetDialogOpen = true;
  }

  async function deleteRules(ids: number[]) {
    if (!profileStore.profile || action.saving) return;
    await action.run(() => writes.run(async () => {
      await coreUpdateRules(rules.filter((r) => r.kind === "FINAL" || !ids.includes(r.id)));
      await profileStore.refresh();
      selectedRuleIds = selectedRuleIds.filter((id) => !ids.includes(id));
    }));
  }

  async function deleteTempRules(ids: number[]) {
    if (!profileStore.profile || action.saving) return;
    await action.run(() => writes.run(async () => {
      await coreUpdateTempRules(tempRules.filter((r) => !ids.includes(r.id)));
      await profileStore.refresh();
      selectedTempRuleIds = selectedTempRuleIds.filter((id) => !ids.includes(id));
    }));
  }

  async function deleteRulesets(ids: string[]) {
    if (!profileStore.profile || action.saving) return;
    await action.run(() => writes.run(async () => {
      await coreUpdateRulesets(ruleSets.filter((r) => !ids.includes(r.id)));
      await profileStore.refresh();
      rulesetSelection.replace(rulesetSelection.ids.filter((id) => !ids.includes(id)));
    }));
  }

  async function handleDelete() {
    if (tab === "rulesets") await deleteRulesets(rulesetSelection.ids);
    else if (tab === "temp") await deleteTempRules(selectedTempRules.map((r) => r.id));
    else await deleteRules(selectedRules.map((r) => r.id));
  }

  async function handlePromote() {
    if (selectedTempRules.length !== 1) return;
    const id = selectedTempRules[0].id;
    await action.run(async () => {
      await corePromoteTempRule(id);
      await profileStore.refresh();
      selectedTempRuleId = tempRules[0]?.id ?? null;
      tab = "rules";
    }, "Promoted to permanent rule.");
  }

  async function handleDialogSave(data: RuleFormSchema) {
    if (dialogTemp) {
      const expires_at = data.expires_at ?? Date.now() + 3_600_000;
      const session = expires_at === 0;
      let next: TempRule[];
      if (dialogMode === "create") {
        const persisted = tempRules.filter((r) => !r.session);
        const nextId =
          persisted.length > 0
            ? Math.max(...persisted.map((r) => r.id)) + 1
            : 1;
        const newRule: TempRule = {
          id: nextId,
          kind: data.kind as RuleType,
          value: data.value,
          outbound: data.outbound,
          comment: data.comment ?? "",
          expires_at,
          enabled: true,
          session,
        };
        next = [newRule, ...tempRules];
        selectedTempRuleId = nextId;
      } else if (dialogMode === "edit" && editingRule) {
        next = tempRules.map((r) =>
          r.id !== editingRule!.id
            ? r
            : {
                ...r,
                kind: data.kind as RuleType,
                value: data.value,
                outbound: data.outbound,
                comment: data.comment ?? "",
                expires_at,
                session,
              },
        );
      } else {
        dialogOpen = false;
        return;
      }
      dialogOpen = false;
      await persistTempRules(next);
      return;
    }

    let next: Rule[];
    const regular = rules.filter((r) => r.kind !== "FINAL");
    const final = rules.filter((r) => r.kind === "FINAL");
    if (dialogMode === "create") {
      const nextId = regular.length
        ? Math.max(...regular.map((r) => r.id)) + 1
        : 1;
      const newRule: Rule = {
        id: nextId,
        kind: data.kind as RuleType,
        value: data.value,
        outbound: data.outbound,
        comment: data.comment ?? "",
        enabled: true,
      };
      next = [newRule, ...regular, ...final];
      selectedRuleId = nextId;
    } else if (dialogMode === "edit" && editingRule) {
      next = rules.map((r) =>
        r.id !== editingRule!.id
          ? r
          : {
              ...r,
              kind: data.kind as RuleType,
              value: data.value,
              outbound: data.outbound,
              comment: data.comment ?? "",
            },
      );
    } else {
      dialogOpen = false;
      return;
    }
    dialogOpen = false;
    await persistRules(next);
  }


  async function handleRulesetDialogSave(data: RulesetDraftPayload) {
    let next: RuleSet[];
    if (rulesetDialogMode === "create") {
      const id =
        typeof crypto !== "undefined" && crypto.randomUUID
          ? crypto.randomUUID()
          : `tmp-${Date.now()}`;
      const tag = `ruleset-${id.replace(/-/g, "").slice(0, 8)}`;
      const rs: RuleSet = {
        id,
        tag,
        type: data.type,
        source: data.source,
        action: data.action,
        comment: data.comment,
        download_policy: data.download_policy ?? globalDownloadPolicy,
        enabled: true,
        name: data.name ?? null,
      };
      next = [...ruleSets, rs];
      selectedRulesetId = id;
    } else if (editingRuleset) {
      next = ruleSets.map((r) =>
        r.id !== editingRuleset!.id
          ? r
          : {
              ...r,
              type: data.type,
              source: data.source,
              action: data.action,
              comment: data.comment,
              name: data.name ?? r.name,
              download_policy:
                data.type === "remote"
                  ? (data.download_policy ?? r.download_policy ?? globalDownloadPolicy)
                  : null,
            },
      );
    } else {
      throw new Error("The ruleset is no longer available.");
    }
    await persistRuleSets(next);
  }

  async function setRulesEnabled(ids: number[], enabled: boolean) {
    if (!profileStore.profile || action.saving) return;
    await action.run(() => writes.run(async () => {
      const targets = rules.filter((r) => r.kind !== "FINAL" && ids.includes(r.id) && (r.enabled !== false) !== enabled);
      if (targets.length === 0) return;
      await coreUpdateRules(rules.map((r) => targets.some((target) => target.id === r.id) ? { ...r, enabled } : r));
      await profileStore.refresh();
    }), undefined, { quiet: true });
  }

  async function setTempRulesEnabled(ids: number[], enabled: boolean) {
    if (!profileStore.profile || action.saving) return;
    await action.run(() => writes.run(async () => {
      const targets = tempRules.filter((r) => ids.includes(r.id) && (r.enabled !== false) !== enabled);
      if (targets.length === 0) return;
      await coreUpdateTempRules(tempRules.map((r) => targets.some((target) => target.id === r.id) ? { ...r, enabled } : r));
      await profileStore.refresh();
    }), undefined, { quiet: true });
  }

  async function setRulesetsEnabled(ids: string[], enabled: boolean) {
    if (!profileStore.profile || action.saving) return;
    await action.run(() => writes.run(async () => {
      await coreUpdateRulesets(ruleSets.map((r) => ids.includes(r.id) ? { ...r, enabled } : r));
      await profileStore.refresh();
    }), undefined, { quiet: true });
  }

  async function setSelectedEnabled(enabled: boolean) {
    if (tab === "temp") await setTempRulesEnabled(selectedTempRules.map((r) => r.id), enabled);
    else if (tab === "rules") await setRulesEnabled(selectedRules.map((r) => r.id), enabled);
    else await setRulesetsEnabled(rulesetSelection.ids, enabled);
  }

  async function toggleRuleEnabled(id: number, enabled: boolean) { await setRulesEnabled([id], enabled); }
  async function toggleTempEnabled(id: number, enabled: boolean) { await setTempRulesEnabled([id], enabled); }
  async function toggleRulesetEnabled(id: string, enabled: boolean) { await setRulesetsEnabled([id], enabled); }

  async function reorderRules(newRules: Rule[]) {
    if (search.trim().length > 0) return;
    await persistRules(newRules);
  }

  async function reorderTempRules(newRules: TempRule[]) {
    if (search.trim().length > 0) return;
    await persistTempRules(newRules);
  }
</script>

<div class="flex flex-col gap-4" style="height: calc(100vh - 6rem);">
  <Tabs.Root
    value={tab}
    onValueChange={(v) => {
      if (v === "rules" || v === "temp" || v === "rulesets") tab = v;
    }}
    class="flex min-h-0 flex-1 flex-col gap-3"
  >
    <div class="flex items-center justify-between">
      <Tabs.List variant="primary">
        <Tabs.Trigger value="rules">Standard Rules</Tabs.Trigger>
        <Tabs.Trigger value="temp">Temporary Rules</Tabs.Trigger>
        <Tabs.Trigger value="rulesets">Ruleset</Tabs.Trigger>
      </Tabs.List>

      <div class="flex items-center gap-2">
        {#if tab === "rules" || tab === "temp"}
          <div class="relative w-full max-w-sm">
            <InputGroup.Root variant="backless">
              <InputGroup.Input placeholder="Search..." bind:value={search} />
              <InputGroup.Addon>
                <HugeiconsIcon icon={Search01Icon} />
              </InputGroup.Addon>
            </InputGroup.Root>
          </div>
        {/if}
        <RuleToolbar
          bind:search
          saving={action.saving}
          hasProfile={!!profileStore.profile}
          {canPromote}
          {tab}
          rulesetsOutbound={globalDownloadPolicy}
          onRulesetsOutboundChange={updateGlobalDownloadPolicy}
          actionGroups={rulesetsActionGroups}
          onAddRule={handleAddClick}
          onAddTempRule={handleAddTempClick}
          onAddRuleset={handleAddRuleset}
          onPromote={handlePromote}
        />
      </div>
    </div>

    <Tabs.Content
      value="rules"
      class="flex min-h-0 flex-1 flex-col data-[state=inactive]:hidden overflow-hidden"
    >
      <ScrollArea
        class="h-full w-full **:data-[slot=scroll-area-scrollbar]:hidden"
      >
        <div class="w-full">
          {#if profileStore.loading}
            <div class="w-full text-sm">
              <div class="sticky top-0 uppercase text-xs bg-background z-20 grid grid-cols-[32px_44px_192px_200px_160px_1fr] px-2 border-b border-border/70">
                <div class="h-8"></div>
                <div class="h-8"></div>
                <div class="h-8 flex items-center font-medium text-muted-foreground">Type</div>
                <div class="h-8 flex items-center font-medium text-muted-foreground">Value</div>
                <div class="h-8 flex items-center font-medium text-muted-foreground">Action</div>
                <div class="h-8 flex items-center font-medium text-muted-foreground">Comment</div>
              </div>
              <div class="p-2 flex flex-col gap-2 animate-in fade-in duration-300">
                {#each Array(6) as _, i (i)}
                  <div class="h-11 grid grid-cols-[32px_44px_192px_200px_160px_1fr] items-center px-2">
                    <div></div>
                    <Skeleton class="size-4 rounded-md" />
                    <Skeleton class="h-5 w-24 rounded-md" />
                    <Skeleton class="h-4 w-32" />
                    <Skeleton class="h-5 w-20 rounded-md" />
                    <Skeleton class="h-4 w-28" />
                  </div>
                {/each}
              </div>
            </div>
          {:else}
            <RuleTable
              rules={filteredRules}
              selectedRuleId={selectedRuleId}
              {selectedRuleIds}
              onSelectionChange={(ids) => (selectedRuleIds = ids)}
              onDeleteRule={(id) => deleteRules([id])}
              disabled={action.saving}
              disableReorder={search.trim().length > 0 || action.saving}
              onSelectRule={(id) => (selectedRuleId = id)}
              onEditRule={handleEditClick}
              {getActionName}
              onReorder={(nr) => reorderRules(nr as Rule[])}
              onToggleEnabled={toggleRuleEnabled}
              onAddRule={handleAddClick}
            />
          {/if}
        </div>
      </ScrollArea>
    </Tabs.Content>

    <Tabs.Content
      value="temp"
      class="flex min-h-0 flex-1 flex-col data-[state=inactive]:hidden overflow-hidden"
    >
      <ScrollArea
        class="h-full w-full **:data-[slot=scroll-area-scrollbar]:hidden"
      >
        <div class="w-full">
          {#if profileStore.loading}
            <div class="w-full text-sm">
              <div class="sticky top-0 uppercase text-xs bg-background z-20 grid grid-cols-[32px_44px_192px_200px_160px_160px_1fr] px-2 border-b border-border/70">
                <div class="h-8"></div>
                <div class="h-8"></div>
                <div class="h-8 flex items-center font-medium text-muted-foreground">Type</div>
                <div class="h-8 flex items-center font-medium text-muted-foreground">Value</div>
                <div class="h-8 flex items-center font-medium text-muted-foreground">Action</div>
                <div class="h-8 flex items-center font-medium text-muted-foreground">Expires</div>
                <div class="h-8 flex items-center font-medium text-muted-foreground">Comment</div>
              </div>
              <div class="p-2 flex flex-col gap-2 animate-in fade-in duration-300">
                {#each Array(4) as _, i (i)}
                  <div class="h-11 grid grid-cols-[32px_44px_192px_200px_160px_160px_1fr] items-center px-2">
                    <div></div>
                    <Skeleton class="size-4 rounded-md" />
                    <Skeleton class="h-5 w-24 rounded-md" />
                    <Skeleton class="h-4 w-32" />
                    <Skeleton class="h-5 w-20 rounded-md" />
                    <Skeleton class="h-4 w-28" />
                    <Skeleton class="h-4 w-20" />
                  </div>
                {/each}
              </div>
            </div>
          {:else}
            <RuleTable
              rules={filteredTempRules}
              selectedRuleId={selectedTempRuleId}
              selectedRuleIds={selectedTempRuleIds}
              onSelectionChange={(ids) => (selectedTempRuleIds = ids)}
              onDeleteRule={(id) => deleteTempRules([id])}
              disabled={action.saving}
              showExpires
              disableReorder={search.trim().length > 0 || action.saving}
              onSelectRule={(id) => (selectedTempRuleId = id)}
              onEditRule={handleEditTempClick}
              {getActionName}
              onReorder={(nr) => reorderTempRules(nr as TempRule[])}
              onToggleEnabled={toggleTempEnabled}
              onAddRule={handleAddTempClick}
            />
          {/if}
        </div>
      </ScrollArea>
    </Tabs.Content>


    <Tabs.Content
      value="rulesets"
      class="flex min-h-0 flex-1 flex-col data-[state=inactive]:hidden overflow-hidden"
    >
      <ScrollArea
        class="h-full w-full **:data-[slot=scroll-area-scrollbar]:hidden"
      >
        <div class="w-full">
          {#if profileStore.loading}
            <div class="w-full text-sm">
              <div class="sticky top-0 uppercase text-xs bg-background z-20 grid grid-cols-[44px_100px_1fr_120px_1fr_160px_160px_1fr] px-2 border-b border-border/70">
                <div class="h-8"></div>
                <div class="h-8 flex items-center font-medium text-muted-foreground px-2">Type</div>
                <div class="h-8 flex items-center font-medium text-muted-foreground px-2">Name</div>
                <div class="h-8 flex items-center font-medium text-muted-foreground px-2">Status</div>
                <div class="h-8 flex items-center font-medium text-muted-foreground px-2">Source</div>
                <div class="h-8 flex items-center font-medium text-muted-foreground px-2">Download via</div>
                <div class="h-8 flex items-center font-medium text-muted-foreground px-2">Action</div>
                <div class="h-8 flex items-center font-medium text-muted-foreground px-2">Comment</div>
              </div>
              <div class="p-2 flex flex-col gap-2 animate-in fade-in duration-300">
                {#each Array(4) as _, i (i)}
                  <div class="h-11 grid grid-cols-[44px_100px_1fr_120px_1fr_160px_160px_1fr] items-center px-2">
                    <div class="flex items-center justify-center">
                      <Skeleton class="size-4 rounded-md" />
                    </div>
                    <div class="px-2"><Skeleton class="h-5 w-16 rounded-md" /></div>
                    <div class="px-2"><Skeleton class="h-4 w-28" /></div>
                    <div class="px-2"><Skeleton class="h-5 w-16 rounded-md" /></div>
                    <div class="px-2"><Skeleton class="h-4 w-32" /></div>
                    <div class="px-2"><Skeleton class="h-4 w-20" /></div>
                    <div class="px-2"><Skeleton class="h-4 w-20" /></div>
                    <div class="px-2"><Skeleton class="h-4 w-24" /></div>
                  </div>
                {/each}
              </div>
            </div>
          {:else}
            <RulesetTable
              {ruleSets}
              selectedId={selectedRulesetId}
              selection={rulesetSelection}
              disabled={action.saving}
              onDelete={(id) => deleteRulesets([id])}
              {getActionName}
              onSelect={(id) => (selectedRulesetId = id)}
              onEdit={handleEditRuleset}
              onToggleEnabled={toggleRulesetEnabled}
              onAddRuleSet={handleAddRuleset}
            />
          {/if}
        </div>
      </ScrollArea>
    </Tabs.Content>
    <SelectionRegion open={selectedRows.length > 0}>
      <RuleSelectionToolbar
        selectedCount={selectedRows.length}
        itemLabel={tab === "rulesets" ? "ruleset" : "rule"}
        canEnable={canEnableSelected}
        canDisable={canDisableSelected}
        saving={action.saving}
        onEnable={() => setSelectedEnabled(true)}
        onDisable={() => setSelectedEnabled(false)}
        onDelete={handleDelete}
      />
    </SelectionRegion>
  </Tabs.Root>
</div>

<RuleDialog
  bind:open={dialogOpen}
  mode={dialogMode}
  rule={editingRule}
  temp={dialogTemp}
  {actionGroups}
  {getActionName}
  onSave={handleDialogSave}
/>


<RulesetDialog
  bind:open={rulesetDialogOpen}
  mode={rulesetDialogMode}
  ruleset={editingRuleset}
  {actionGroups}
  {getActionName}
  onSave={handleRulesetDialogSave}
/>