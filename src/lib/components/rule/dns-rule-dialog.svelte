<script lang="ts">
  import * as Dialog from "$lib/components/ui/dialog/index.js";
  import { Button } from "$lib/components/ui/button/index.js";
  import { Input } from "$lib/components/ui/input/index.js";
  import { Label } from "$lib/components/ui/label/index.js";
  import * as Select from "$lib/components/ui/select/index.js";
  import type {
    DnsRuleKind,
    DnsRuleTarget,
    DnsServer,
    RuleSet,
  } from "$lib/core/types";
  import type { DnsRuleItem } from "./dns-rule-table.svelte";
  import { errorMessage } from "$lib/errors";

  type DnsServerOption = {
    value: string;
    label: string;
  };

  interface Props {
    open: boolean;
    mode?: "create" | "edit";
    rule?: DnsRuleItem | null;
    dnsServers?: DnsServer[];
    ruleSets?: RuleSet[];
    onSave: (
      data: Omit<DnsRuleItem, "id" | "enabled">,
    ) => Promise<void>;
  }

  let {
    open = $bindable(false),
    mode = "create",
    rule = null,
    dnsServers = [],
    ruleSets = [],
    onSave,
  }: Props = $props();

  let kind = $state<DnsRuleKind>("domain");
  let value = $state("");
  let target = $state<DnsRuleTarget>("");
  let comment = $state("");
  let saving = $state(false);
  let error = $state<string | null>(null);

  const kinds: { value: DnsRuleKind; label: string }[] = [
    { value: "domain", label: "Exact Domain" },
    { value: "domain_suffix", label: "Domain Suffix" },
    { value: "ruleset", label: "Rule-set Match" },
  ];

  function serverLocation(server: DnsServer): "Local" | "Remote" {
    return server.detour?.toLowerCase() === "direct" ? "Local" : "Remote";
  }

  function serverLabel(server: DnsServer): string {
    return server.name;
  }

  function rulesetLabel(ruleset: RuleSet): string {
    return ruleset.name && ruleset.name !== ruleset.tag
      ? `${ruleset.name} (${ruleset.tag})`
      : ruleset.tag;
  }

  const serverOptions = $derived<DnsServerOption[]>(
    dnsServers.map((server) => ({
      value: server.tag,
      label: serverLabel(server),
    })),
  );

  const rulesetOptions = $derived(
    ruleSets.filter((ruleset) => ruleset.enabled).map((ruleset) => ({
      value: ruleset.tag,
      label: rulesetLabel(ruleset),
    })),
  );

  const hasValidKind = $derived(
    kinds.some((option) => option.value === kind),
  );
  const hasValidTarget = $derived(
    target === "block" || serverOptions.some((option) => option.value === target),
  );
  const hasValidRuleset = $derived(
    kind !== "ruleset" ||
      rulesetOptions.some((option) => option.value === value.trim()),
  );
  const hasUnavailableTarget = $derived(target !== "" && !hasValidTarget);
  const hasUnavailableRuleset = $derived(
    kind === "ruleset" && value.trim() !== "" && !hasValidRuleset,
  );
  const isValid = $derived(
    hasValidKind && value.trim() !== "" && hasValidTarget && hasValidRuleset,
  );

  function isDnsRuleKind(candidate: string): candidate is DnsRuleKind {
    return kinds.some((option) => option.value === candidate);
  }

  function selectedTargetLabel(): string {
    if (!target) return "Select target...";
    if (target === "block") return "Block";
    return (
      serverOptions.find((option) => option.value === target)?.label ??
      `${target} (missing DNS server)`
    );
  }

  function selectedRulesetLabel(): string {
    if (!value) {
      return rulesetOptions.length > 0
        ? "Select ruleset..."
        : "No enabled rulesets available";
    }

    const enabled = rulesetOptions.find((option) => option.value === value);
    if (enabled) return enabled.label;

    const unavailable = ruleSets.find((ruleset) => ruleset.tag === value);
    return unavailable
      ? `${rulesetLabel(unavailable)} (disabled)`
      : `${value} (missing ruleset)`;
  }

  $effect(() => {
    if (!open) return;
    error = null;

    if (mode === "edit" && rule) {
      kind = rule.kind;
      value = rule.value;
      target = rule.target;
      comment = rule.comment ?? "";
    } else {
      kind = "domain";
      value = "";
      target = dnsServers[0]?.tag ?? "block";
      comment = "";
    }
  });

  async function handleSubmit(e: Event) {
    e.preventDefault();
    if (!isValid || saving) return;

    saving = true;
    error = null;
    try {
      await onSave({
        kind,
        value: value.trim(),
        target,
        comment: comment.trim() || undefined,
      });
      open = false;
    } catch (err) {
      console.error("Failed to save DNS rule:", err);
      error = errorMessage(err);
    } finally {
      saving = false;
    }
  }
</script>

<Dialog.Root bind:open>
  <Dialog.Content class="sm:max-w-md flex flex-col">
    <form onsubmit={handleSubmit} class="flex flex-col gap-4">
      <Dialog.Header>
        <Dialog.Title>
          {mode === "create" ? "Add DNS Rule" : "Edit DNS Rule"}
        </Dialog.Title>
        <Dialog.Description>
          Specify traffic matching criteria and select the target DNS resolver.
        </Dialog.Description>
      </Dialog.Header>

      {#if error}
        <div class="rounded bg-red-500/10 p-2 text-sm text-red-500">
          {error}
        </div>
      {/if}

      <div class="flex flex-col gap-4">
        <div class="flex flex-col gap-2">
          <Label>Rule Type</Label>
          <Select.Root
            type="single"
            value={kind}
            disabled={saving}
            onValueChange={(nextKind) => {
              if (!isDnsRuleKind(nextKind)) return;
              kind = nextKind;
              value = "";
            }}
          >
            <Select.Trigger class="w-full justify-between">
              {kinds.find((k) => k.value === kind)?.label}
            </Select.Trigger>
            <Select.Content>
              {#each kinds as k (k.value)}
                <Select.Item value={k.value} label={k.label}>
                  {k.label}
                </Select.Item>
              {/each}
            </Select.Content>
          </Select.Root>
        </div>

        <div class="flex flex-col gap-2">
          <Label for="dns-rule-value">
            {kind === "ruleset"
              ? "Rule-set"
              : kind === "domain_suffix"
                ? "Domain Suffix"
                : "Domain"}
          </Label>

          {#if kind === "ruleset"}
            <Select.Root
              type="single"
              value={value}
              disabled={saving || rulesetOptions.length === 0}
              onValueChange={(nextValue) => (value = nextValue)}
            >
              <Select.Trigger class="w-full justify-between">
                {selectedRulesetLabel()}
              </Select.Trigger>
              <Select.Content>
                {#each rulesetOptions as option (option.value)}
                  <Select.Item value={option.value} label={option.label}>
                    {option.label}
                  </Select.Item>
                {/each}
              </Select.Content>
            </Select.Root>
            {#if hasUnavailableRuleset}
              <p class="text-destructive text-xs">
                This ruleset is missing or disabled. Select an enabled ruleset or
                change the rule type before saving.
              </p>
            {:else if rulesetOptions.length === 0}
              <p class="text-muted-foreground text-xs">
                No enabled rulesets are available.
              </p>
            {/if}
          {:else}
            <Input
              id="dns-rule-value"
              bind:value
              disabled={saving}
              placeholder={kind === "domain" ? "example.com" : ".example.com"}
            />
          {/if}
        </div>

        <div class="flex flex-col gap-2">
          <Label>Target Resolver</Label>
          <Select.Root
            type="single"
            value={target}
            disabled={saving}
            onValueChange={(nextTarget) => (target = nextTarget)}
          >
            <Select.Trigger class="w-full justify-between">
              {selectedTargetLabel()}
            </Select.Trigger>
            <Select.Content>
              {#if serverOptions.length > 0}
                <Select.Group>
                  <Select.Label>DNS Servers</Select.Label>
                  {#each serverOptions as option (option.value)}
                    <Select.Item value={option.value} label={option.label}>
                      {option.label}
                    </Select.Item>
                  {/each}
                </Select.Group>
              {/if}
              <Select.Group>
                <Select.Label>Action</Select.Label>
                <Select.Item value="block" label="Block">Block</Select.Item>
              </Select.Group>
            </Select.Content>
          </Select.Root>
          {#if hasUnavailableTarget}
            <p class="text-destructive text-xs">
              DNS server "{target}" no longer exists. Select an available server
              or Block before saving.
            </p>
          {/if}
        </div>

        <div class="flex flex-col gap-2">
          <Label for="dns-rule-comment">Comment (Optional)</Label>
          <Input
            id="dns-rule-comment"
            bind:value={comment}
            disabled={saving}
            placeholder="e.g. Use a dedicated resolver"
          />
        </div>
      </div>

      <Dialog.Footer>
        <Button
          variant="outline"
          type="button"
          disabled={saving}
          onclick={() => (open = false)}
        >
          Cancel
        </Button>
        <Button type="submit" disabled={!isValid || saving}>
          {saving
            ? "Saving..."
            : mode === "create"
              ? "Add DNS Rule"
              : "Save Changes"}
        </Button>
      </Dialog.Footer>
    </form>
  </Dialog.Content>
</Dialog.Root>
