<script lang="ts">
  import * as Dialog from "$lib/components/ui/dialog/index.js";
  import { Input } from "$lib/components/ui/input/index.js";
  import { Button } from "$lib/components/ui/button/index.js";
  import { Label } from "$lib/components/ui/label/index.js";
  import { profileStore } from "$lib/stores/profile.svelte";
  import { coreUpdateRules } from "$lib/core/api";
  import type { Rule, RuleType } from "$lib/core/types";
  import * as Select from "$lib/components/ui/select/index.js";
  import { errorMessage } from "$lib/errors";

  let {
    open = $bindable(false),
    kind = "DOMAIN" as RuleType,
    value = "",
    label = "",
    onSuccess,
    onError,
  } = $props<{
    open: boolean;
    kind: RuleType;
    value: string;
    label: string;
    onSuccess?: (msg: string) => void;
    onError?: (err: string) => void;
  }>();

  let ruleOutbound = $state("direct");
  let ruleComment = $state("");

  $effect(() => {
    if (open) {
      ruleComment = `Rule for ${value}`;
      ruleOutbound = "direct";
    }
  });

  const actionGroups = $derived.by(() => {
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

  async function save() {
    if (!profileStore.profile) return;
    try {
      const regular = profileStore.profile.rules.filter(
        (r) => r.kind !== "FINAL",
      );
      const final = profileStore.profile.rules.filter(
        (r) => r.kind === "FINAL",
      );
      const nextId = regular.length
        ? Math.max(...regular.map((r) => r.id)) + 1
        : 1;

      const newRule: Rule = {
        id: nextId,
        kind,
        value,
        outbound: ruleOutbound,
        comment: ruleComment,
      };

      profileStore.profile.rules = [newRule, ...regular, ...final];
      await coreUpdateRules(profileStore.profile.rules);
      onSuccess?.(`Rule for ${value} added.`);
      open = false;
    } catch (err) {
      onError?.(`Failed to add rule: ${errorMessage(err)}`);
    }
  }
</script>

<Dialog.Root bind:open>
  <Dialog.Content class="sm:max-w-lg flex flex-col">
    <Dialog.Header>
      <Dialog.Title>Add Rule</Dialog.Title>
      <Dialog.Description
        class="truncate"
        title="{label} rule for {value} ({kind})"
      >
        {label} rule for <strong class="break-all">{value}</strong> (<code
          >{kind}</code
        >).
      </Dialog.Description>
    </Dialog.Header>
    <div class="flex flex-col gap-4">
      <div class="flex flex-col gap-2">
        <Label class="text-right">Match</Label>
        <Input value={`${kind} = ${value}`} disabled />
      </div>
      <div class="flex flex-col gap-2">
        <Label class="text-right">Outbound</Label>
        <Select.Root
          type="single"
          value={ruleOutbound}
          onValueChange={(v) => {
            ruleOutbound = v;
          }}
        >
          <Select.Trigger class="w-full justify-between">
            {actionGroups
              .flatMap((group) => group.options)
              .find((item) => item.tag === ruleOutbound)?.name}
          </Select.Trigger>
          <Select.Content class="max-h-64">
            {#each actionGroups as group (group.label)}
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
      <div class="flex flex-col gap-2">
        <Label for="rule-comment" class="text-right">Comment</Label>
        <Input id="rule-comment" bind:value={ruleComment} />
      </div>
    </div>
    <Dialog.Footer>
      <Button variant="outline" onclick={() => (open = false)}>Cancel</Button>
      <Button onclick={save}>Save Rule</Button>
    </Dialog.Footer>
  </Dialog.Content>
</Dialog.Root>
