<script lang="ts">
  import * as Dialog from "$lib/components/ui/dialog/index.js";
  import { Input } from "$lib/components/ui/input/index.js";
  import { Button } from "$lib/components/ui/button/index.js";
  import { Label } from "$lib/components/ui/label/index.js";
  import * as Select from "$lib/components/ui/select/index.js";
  import { profileStore } from "$lib/stores/profile.svelte";
  import { coreUpdateRules } from "$lib/core/api";
  import type { Rule } from "$lib/core/types";
  import { errorMessage } from "$lib/errors";

  let { 
    open = $bindable(false),
    processName = "",
    processPath = "",
    onSuccess,
    onError
  } = $props<{
    open: boolean;
    processName: string;
    processPath: string;
    onSuccess?: (msg: string) => void;
    onError?: (err: string) => void;
  }>();

  let ruleOutbound = $state("direct");
  let ruleComment = $state("");

  // Update defaults when opened
  $effect(() => {
    if (open && processName) {
      ruleComment = `Bypass/proxy rule for ${processName}`;
      ruleOutbound = "direct";
    }
  });

  const actionGroups = $derived.by(() => {
    if (!profileStore.profile) return [
      {
        label: "Built-in",
        options: [
          { tag: "direct", name: "Direct" },
          { tag: "block", name: "Block" }
        ]
      }
    ];

    const builtin = [
      { tag: "direct", name: "Direct" },
      { tag: "block", name: "Block" }
    ];

    const policies = profileStore.profile.policies
      .filter((p) => p.tag !== "root-policy")
      .map((p) => ({
        tag: p.tag,
        name: p.name || p.tag
      }));

    const proxies = profileStore.profile.proxies
      .filter((p) => p.enabled)
      .map((p) => ({
        tag: p.tag,
        name: p.title || p.tag
      }));

    return [
      { options: builtin },
      { label: "Policies", options: policies },
      { label: "Proxies", options: proxies }
    ];
  });

  /** Derive the rule kind + value from context.
   *  Process tab = broad: if path is inside a .app bundle, match the whole bundle
   *  via PROCESS-PATH-REGEX; otherwise exact path via PROCESS-PATH. */
  function buildProcessRule(path: string): { kind: Rule["kind"]; value: string } {
    if (!path) return { kind: "PROCESS-NAME", value: processName };
    const bundleMatch = path.match(/^(.*\.app)\//);
    if (bundleMatch) {
      const escaped = bundleMatch[1].replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
      return { kind: "PROCESS-PATH-REGEX", value: `^${escaped}/` };
    }
    return { kind: "PROCESS-PATH", value: path };
  }

  const rulePreview = $derived(buildProcessRule(processPath));

  async function handleAddRuleSave() {
    if (!profileStore.profile) return;
    try {
      const regularRules = profileStore.profile.rules.filter((rule) => rule.kind !== "FINAL");
      const finalRules = profileStore.profile.rules.filter((rule) => rule.kind === "FINAL");

      let nextId = 1;
      if (regularRules.length > 0) {
        nextId = Math.max(...regularRules.map((r) => r.id)) + 1;
      }

      const { kind, value } = rulePreview;
      const newRule: Rule = {
        id: nextId,
        kind,
        value,
        outbound: ruleOutbound,
        comment: ruleComment,
      };

      profileStore.profile.rules = [newRule, ...regularRules, ...finalRules];
      await coreUpdateRules(profileStore.profile.rules);
      
      onSuccess?.(`Rule for ${processName} added successfully.`);
      open = false;
    } catch (err) {
      onError?.(`Failed to add rule: ${errorMessage(err)}`);
    }
  }
</script>

<Dialog.Root bind:open>
  <Dialog.Content class="sm:max-w-106.25">
    <Dialog.Header>
      <Dialog.Title>Add Bypass/Proxy Rule</Dialog.Title>
      <Dialog.Description>
        Create a rule for connections from <strong>{processName}</strong>.
      </Dialog.Description>
    </Dialog.Header>
    <div class="grid gap-4 py-4">
      <div class="grid grid-cols-4 items-center gap-4">
        <Label for="processName" class="text-right">Process</Label>
        <Input id="processName" value={processName} class="col-span-3 h-9" disabled />
      </div>
      <div class="grid grid-cols-4 items-center gap-4">
        <Label for="outbound" class="text-right">Outbound</Label>
        <div class="col-span-3">
          <Select.Root
            type="single"
            value={ruleOutbound}
            onValueChange={(v) => {
              ruleOutbound = v;
            }}
          >
            <Select.Trigger class="w-full justify-between h-9">
              {actionGroups
                .flatMap((group) => group.options)
                .find((item) => item.tag === ruleOutbound)?.name}
            </Select.Trigger>
            <Select.Content class="max-h-64">
              {#each actionGroups as group (group.label || "builtin")}
                <Select.Group>
                  {#if group.label}
                    <Select.Label>{group.label}</Select.Label>
                  {/if}
                  {#each group.options as option (option.tag)}
                    <Select.Item value={option.tag} label={option.name}>
                      {option.name}
                    </Select.Item>
                  {/each}
                </Select.Group>
              {/each}
            </Select.Content>
          </Select.Root>
        </div>
      </div>
      <div class="grid grid-cols-4 items-center gap-4">
        <Label for="comment" class="text-right">Comment</Label>
        <Input id="comment" bind:value={ruleComment} class="col-span-3 h-9" placeholder="Comment for this rule" />
      </div>
    </div>
    <Dialog.Footer>
      <Button variant="outline" onclick={() => open = false}>Cancel</Button>
      <Button onclick={handleAddRuleSave}>Save Rule</Button>
    </Dialog.Footer>
  </Dialog.Content>
</Dialog.Root>
