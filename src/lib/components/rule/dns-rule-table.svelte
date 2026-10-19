<script lang="ts">
  import type { RowSelection } from "$lib/composables/row-selection.svelte";
  import { Checkbox } from "$lib/components/ui/checkbox";
  import { Badge } from "$lib/components/ui/badge/index.js";
  import * as Select from "$lib/components/ui/select/index.js";
  import type {
    DnsRuleDto,
    DnsRuleKind,
    DnsRuleTarget,
    DnsServer,
    OutboundMode,
  } from "$lib/core/types";
  import { cn } from "$lib/utils";
  import { dnsRuleTargetLabel, LOCAL_DNS_SERVER_TAG } from "$lib/core/dns-servers";
  import DnsRowActions from "./dns-row-actions.svelte";
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
    selection: RowSelection<string>;
    onSelect: (id: string) => void;
    onEdit: (id: string) => void;
    onDelete: (id: string) => void;
    onToggleEnabled?: (id: string, enabled: boolean) => void;
    /** `dns.final`: the resolver for queries no rule matched. Null = local. */
    finalTarget?: string | null;
    fakeIp?: boolean;
    outboundMode?: OutboundMode;
    savedPolicy?: boolean;
    onFinalChange?: (target: string | null) => void;
    disabled?: boolean;
  }

  let {
    rules,
    dnsServers = [],
    selectedId,
    selection,
    onSelect,
    onEdit,
    onDelete,
    onToggleEnabled,
    finalTarget = null,
    fakeIp = false,
    outboundMode = "rule",
    savedPolicy = false,
    onFinalChange,
    disabled = false,
  }: Props = $props();

  const GRID = "grid-cols-[32px_44px_110px_1fr_160px_1fr_40px]";

  function targetBadgeStyle(target: DnsRuleTarget) {
    return target === "block"
      ? "border-red-500/20 bg-red-500/10 text-red-500"
      : "border-sky-500/20 bg-sky-500/10 text-sky-500";
  }

  function targetLabel(target: DnsRuleTarget) {
    return dnsRuleTargetLabel(target, dnsServers);
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
  const finalValue = $derived(
    !finalTarget || finalTarget === LOCAL_DNS_SERVER_TAG ? LOCAL_VALUE : finalTarget,
  );
  const finalLabel = $derived.by(() => {
    if (!finalTarget || finalTarget === LOCAL_DNS_SERVER_TAG) return "Local (System)";
    const server = dnsServers.find((s) => s.tag === finalTarget);
    return server?.name || `${finalTarget} (missing)`;
  });
  const fakeIpEffective = $derived(fakeIp && outboundMode !== "direct");
  const finalDescription = $derived(
    `${savedPolicy ? "Saved settings. " : ""}Final never overrides Hosts or matching DNS rules. Local names use Local (System). ${fakeIpEffective ? "Unmatched A / AAAA queries use FakeIP before Final." : fakeIp && outboundMode === "direct" ? "FakeIP is bypassed in Direct mode." : "Final resolves unmatched queries."}`,
  );
</script>

<div class="w-full text-sm">
  <div
    class="sticky top-0 uppercase text-xs bg-background z-20 grid {GRID} px-2 border-b border-border/70"
  >
    <div class="h-8 flex items-center justify-center">
      <Checkbox aria-label="Select all visible DNS rules" {disabled}
        checked={rules.length > 0 && rules.every((r) => selection.ids.includes(r.id))}
        indeterminate={rules.some((r) => selection.ids.includes(r.id)) && !rules.every((r) => selection.ids.includes(r.id))}
        onCheckedChange={(checked) => selection.selectAll(checked, rules.map((r) => r.id))} />
    </div>
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
    <div class="sr-only">Actions</div>
  </div>
  <div class="p-0">
    {#each rules as r, i (r.id)}
      {#if i > 0}<div class="h-px w-full bg-border/40"></div>{/if}
      <DnsRowActions
        label={`DNS rule ${r.value}`}
        selected={selection.ids.includes(r.id)}
        {disabled}
        class="h-12 grid {GRID} items-center rounded-xl hover:bg-muted/30 data-[state=selected]:bg-muted/40 transition-colors px-2 cursor-pointer {r.enabled === false ? 'opacity-50' : ''}"
        onSelect={(event) => { onSelect(r.id); selection.select(r.id, event, rules.map((r) => r.id)); }}
        onContextSelect={() => { onSelect(r.id); selection.select(r.id); }}
        enabled={r.enabled !== false}
        onToggleEnabled={() => onToggleEnabled?.(r.id, r.enabled === false)}
        onEdit={() => onEdit(r.id)}
        onDelete={() => onDelete(r.id)}
      >
        <div class="flex items-center justify-center">
          <Checkbox aria-label={`Select DNS rule ${r.value}`} {disabled}
            checked={selection.ids.includes(r.id)} onCheckedChange={(checked) => selection.toggle(r.id, checked)} />
        </div>
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
      </DnsRowActions>
    {/each}

    <!--
      `dns.final` as the last row. It is always in effect and cannot be
      disabled or deleted: the core resolves every unmatched query with it, and
      leaving it unset lets the core promote whichever server happens to be
      first — which once made the hosts table resolve everything.
    -->
    {#if rules.length > 0}<div class="h-px w-full bg-border/70"></div>{/if}
    <div
      class="h-12 grid {GRID} items-center bg-muted/20 px-2"
      title={finalDescription}
    >
      <div></div>
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
        {fakeIpEffective ? "Remaining queries (after FakeIP)" : "Everything else (unmatched queries)"}
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
        {savedPolicy ? "Saved · " : ""}{fakeIpEffective ? "A / AAAA use FakeIP before Final" : fakeIp && outboundMode === "direct" ? "FakeIP bypassed in Direct mode" : "Fallback — specific rules take priority"}
      </div>
      <div></div>
    </div>
  </div>
</div>
