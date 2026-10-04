<script lang="ts">
  import { Button } from "$lib/components/ui/button/index.js";
  import * as DropdownMenu from "$lib/components/ui/dropdown-menu/index.js";
  import * as Select from "$lib/components/ui/select/index.js";
  import {
    MinusSignIcon,
    PlusSignIcon,
    Tick02Icon,
  } from "@hugeicons/core-free-icons";
  import { HugeiconsIcon } from "@hugeicons/svelte";

  export type ActionGroupOption = { tag: string; name: string };
  export type ActionGroup = { label?: string; options: ActionGroupOption[] };

  interface Props {
    search: string;
    saving: boolean;
    hasProfile: boolean;
    canDelete: boolean;
    canPromote: boolean;
    tab: "rules" | "temp" | "rulesets";
    // Ruleset specific
    rulesetsOutbound?: string;
    onRulesetsOutboundChange?: (val: string) => void;
    actionGroups?: ActionGroup[];

    onAddRule: () => void;
    onAddTempRule: () => void;
    onAddRuleset: () => void;
    onDelete: () => void;
    onPromote?: () => void;
  }

  let {
    search = $bindable(),
    saving,
    hasProfile,
    canDelete,
    canPromote,
    tab,
    rulesetsOutbound = "direct",
    onRulesetsOutboundChange,
    actionGroups = [],
    onAddRule,
    onAddTempRule,
    onAddRuleset,
    onDelete,
    onPromote,
  }: Props = $props();

  function getOptionLabel(tag: string) {
    for (const group of actionGroups) {
      const opt = group.options.find((o) => o.tag === tag);
      if (opt) return opt.name;
    }
    return tag;
  }
</script>

<div class="flex items-center gap-2">
  {#if tab === "rulesets" && actionGroups.length > 0}
    <div class="hidden sm:flex items-center gap-1.5 ml-2 mr-1">
      <span class="text-sm text-muted-foreground whitespace-nowrap"
        >Download with:</span
      >
      <Select.Root
        type="single"
        value={rulesetsOutbound}
        onValueChange={(v) => {
          if (v) onRulesetsOutboundChange?.(v);
        }}
        disabled={!hasProfile || saving}
      >
        <Select.Trigger>
          <span>{getOptionLabel(rulesetsOutbound)}</span>
        </Select.Trigger>
        <Select.Content>
          {#each actionGroups as group (group.label || "default")}
            {#if group.options.length > 0}
              {#each group.options as opt (opt.tag)}
                <Select.Item value={opt.tag} label={opt.name}>
                  {opt.name}
                </Select.Item>
              {/each}
            {/if}
          {/each}
        </Select.Content>
      </Select.Root>
    </div>
  {/if}
  {#if tab === "temp"}
    <Button
      variant="outline"
      disabled={!canPromote || saving}
      onclick={onPromote}
    >
      <HugeiconsIcon icon={Tick02Icon} class="size-4" />
      Make permanent
    </Button>
  {/if}
  <DropdownMenu.Root>
    <DropdownMenu.Trigger>
      {#snippet child({ props })}
        <Button
          variant="outline"
          class="bg-card/70!"
          size="icon"
          disabled={!hasProfile || saving}
          {...props}
        >
          <HugeiconsIcon icon={PlusSignIcon} />
          <span class="sr-only">Add</span>
        </Button>
      {/snippet}
    </DropdownMenu.Trigger>
    <DropdownMenu.Content align="end">
      <DropdownMenu.Item onclick={onAddRule}>Standard Rule</DropdownMenu.Item>
      <DropdownMenu.Item onclick={onAddTempRule}>
        Temporary Rule
      </DropdownMenu.Item>

      <DropdownMenu.Item onclick={onAddRuleset}>Ruleset</DropdownMenu.Item>
    </DropdownMenu.Content>
  </DropdownMenu.Root>
  <Button
    variant="outline"
    size="icon"
    class="bg-card/70!"
    disabled={!canDelete || saving}
    onclick={onDelete}
  >
    <HugeiconsIcon icon={MinusSignIcon} />
    <span class="sr-only">Delete selected</span>
  </Button>
</div>
