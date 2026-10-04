<script lang="ts">
  import { Badge } from "$lib/components/ui/badge/index.js";
  import * as Select from "$lib/components/ui/select/index.js";
  import type {
    DnsRuleDto,
    DnsRuleKind,
    DnsRuleTarget,
    DnsServer,
  } from "$lib/core/types";
  import { cn } from "$lib/utils";
  import CellEnabled from "./cell-enabled.svelte";

  export type { DnsRuleKind, DnsRuleTarget };
  export type DnsRuleItem = Omit<DnsRuleDto, "comment"> & {
    comment?: string;
  };

  /** Sentinel for the built-in local resolver: `dns.final` is null/absent. */
  const LOCAL_VALUE = "__local__";

  interface Props {
    rules: DnsRuleItem[];
    dnsServers?: DnsServer[];
    selectedId: string | null;
    onSelect: (id: string) => void;
    onEdit: (id: string) => void;
    onToggleEnabled?: (id: string, enabled: boolean) => void;
    /** `dns.final`: the resolver for queries no rule matched. Null = local. */
    finalTarget?: string | null;
    onFinalChange?: (target: string | null) => void;
    disabled?: boolean;
  }

  let {
    rules,
    dnsServers = [],
    selectedId,
    onSelect,
    onEdit,
    onToggleEnabled,
    finalTarget = null,
    onFinalChange,
    disabled = false,
  }: Props = $props();

  const GRID = "grid-cols-[44px_110px_1fr_160px_1fr]";

  function targetBadgeStyle(target: DnsRuleTarget) {
    return target === "block"
      ? "border-red-500/20 bg-red-500/10 text-red-500"
      : "border-sky-500/20 bg-sky-500/10 text-sky-500";
  }

  function targetLabel(target: DnsRuleTarget) {
    if (target === "block") return "Block";
    const server = dnsServers.find((s) => s.tag === target);
    return server?.name || target;
  }

  function kindLabel(kind: DnsRuleKind) {
    switch (kind) {
      case "domain":
        return "DOMAIN";
      case "domain_suffix":
        return "SUFFIX";
      case "ruleset":
        return "RULESET";
    }
  }

  // A stale tag (server deleted while it was `final`) still renders, so the row
  // never silently lies about which resolver the core will use.
  const finalValue = $derived(finalTarget ?? LOCAL_VALUE);
  const finalLabel = $derived.by(() => {
    if (!finalTarget) return "Local (System)";
    const server = dnsServers.find((s) => s.tag === finalTarget);
    return server?.name || `${finalTarget} (missing)`;
  });
</script>

<div class="w-full text-sm">
  <div
    class="sticky top-0 uppercase text-xs bg-background z-20 grid {GRID} px-2 border-b border-border/70"
  >
    <div class="h-8 flex items-center font-medium text-muted-foreground"></div>
    <div class="h-8 flex items-center font-medium text-muted-foreground px-2">
      Type
    </div>
    <div class="h-8 flex items-center font-medium text-muted-foreground px-2">
      Pattern / Value
    </div>
    <div class="h-8 flex items-center font-medium text-muted-foreground px-2">
      Resolver Target
    </div>
    <div class="h-8 flex items-center font-medium text-muted-foreground px-2">
      Comment
    </div>
  </div>
  <div class="p-0">
    {#if rules.length === 0}
      <div class="text-muted-foreground p-8 text-center text-sm">
        No DNS rules configured yet.
      </div>
    {:else}
      {#each rules as r, i (r.id)}
        {#if i > 0}<div class="h-px w-full bg-border/40"></div>{/if}
        <!-- svelte-ignore a11y_click_events_have_key_events -->
        <!-- svelte-ignore a11y_no_static_element_interactions -->
        <div
          data-state={selectedId === r.id ? "selected" : undefined}
          class="h-12 grid {GRID} items-center rounded-xl hover:bg-muted/30 data-[state=selected]:bg-muted/40 transition-colors px-2 cursor-pointer {r.enabled ===
          false
            ? 'opacity-50'
            : ''}"
          onclick={() => onSelect(r.id)}
          ondblclick={() => onEdit(r.id)}
        >
          <div class="px-2 flex items-center justify-center">
            <CellEnabled
              checked={r.enabled !== false}
              {disabled}
              onChange={(v) => onToggleEnabled?.(r.id, v)}
            />
          </div>
          <div class="px-2 flex items-center">
            <Badge
              variant="secondary"
              class="text-muted-foreground uppercase font-mono"
            >
              {kindLabel(r.kind)}
            </Badge>
          </div>
          <div
            class="px-2 truncate font-mono font-medium text-foreground"
            title={r.value}
          >
            {r.value}
          </div>
          <div class="px-2 flex items-center">
            <Badge class={cn(targetBadgeStyle(r.target), "font-mono")}>
              {targetLabel(r.target)}
            </Badge>
          </div>
          <div class="px-2 truncate italic text-muted-foreground/60">
            {r.comment || "—"}
          </div>
        </div>
      {/each}
    {/if}

    <!--
      `dns.final` as the last row. It is always in effect and cannot be
      disabled or deleted: the core resolves every unmatched query with it, and
      leaving it unset lets the core promote whichever server happens to be
      first — which once made the hosts table resolve everything.
    -->
    <div class="h-px w-full bg-border/70"></div>
    <div
      class="h-12 grid {GRID} items-center bg-muted/20 px-2"
      title="Always last. Resolves every query no rule above matched."
    >
      <div class="px-2 flex items-center justify-center">
        <CellEnabled checked disabled onChange={() => {}} />
      </div>
      <div class="px-2 flex items-center">
        <Badge
          variant="secondary"
          class="text-muted-foreground uppercase font-mono"
        >
          Final
        </Badge>
      </div>
      <div class="px-2 truncate text-muted-foreground">
        Everything else (unmatched queries)
      </div>
      <div class="px-2 flex items-center">
        <Select.Root
          type="single"
          value={finalValue}
          {disabled}
          onValueChange={(next) =>
            onFinalChange?.(next === LOCAL_VALUE ? null : next)}
        >
          <Select.Trigger size="sm" class="w-full justify-between">
            {finalLabel}
          </Select.Trigger>
          <Select.Content>
            <Select.Item value={LOCAL_VALUE} label="Local (System)">
              Local (System)
            </Select.Item>
            {#each dnsServers as server (server.tag)}
              <Select.Item value={server.tag} label={server.name}>
                {server.name}
              </Select.Item>
            {/each}
          </Select.Content>
        </Select.Root>
      </div>
      <div class="px-2 truncate italic text-muted-foreground/60">
        Default resolver — cannot be disabled
      </div>
    </div>
  </div>
</div>
