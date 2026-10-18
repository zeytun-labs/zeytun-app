<script lang="ts">
  import type { RowSelection } from "$lib/composables/row-selection.svelte";
  import { Checkbox } from "$lib/components/ui/checkbox";
  import DnsRowActions from "./dns-row-actions.svelte";
  import type { RuleSet } from "$lib/core/types";
  import { Badge, type BadgeVariant } from "$lib/components/ui/badge/index.js";
  import { notificationStore } from "$lib/stores/notifications.svelte";
  import CellEnabled from "./cell-enabled.svelte";
  import * as Empty from "$lib/components/ui/empty/index.js";
  import { Button } from "$lib/components/ui/button/index.js";
  import { HugeiconsIcon } from "@hugeicons/svelte";
  import { Add01Icon, Layers01Icon } from "@hugeicons/core-free-icons";

  interface Props {
    ruleSets: RuleSet[];
    selectedId: string | null;
    selection: RowSelection<string>;
    disabled?: boolean;
    onDelete: (id: string) => void;
    getActionName: (tag: string) => string;
    onSelect: (id: string) => void;
    onEdit: (id: string) => void;
    onToggleEnabled?: (id: string, enabled: boolean) => void;
    onAddRuleSet?: () => void;
  }

  let {
    ruleSets,
    selectedId,
    selection,
    disabled = false,
    onDelete,
    getActionName,
    onSelect,
    onEdit,
    onToggleEnabled,
    onAddRuleSet,
  }: Props = $props();

  function downloadViaLabel(rs: RuleSet): string {
    // Compile default is "direct" when download_policy null — never fake root-policy.
    const tag = rs.download_policy?.trim() || "direct";
    return getActionName(tag);
  }

  function statusOf(
    rs: RuleSet,
  ): { text: string; variant: BadgeVariant; class?: string; title?: string } {
    if (rs.type === "local") {
      return { text: "Local", variant: "secondary" };
    }
    const st = notificationStore.rulesetStatus(rs.tag);
    if (st === "failed") {
      return {
        text: "Not downloaded",
        variant: "destructive",
        title:
          notificationStore.rulesetError(rs.tag) ??
          "Download failed. Core skipped this ruleset. Retry with another outbound.",
      };
    }
    if (st === "ready") {
      return {
        text: "Ready",
        variant: "secondary",
        class: "bg-lime-500/10 text-lime-700 dark:text-lime-400 border-none",
      };
    }
    if (st === "downloading") {
      return {
        text: "Downloading",
        variant: "secondary",
        class: "text-sky-700 dark:text-sky-400 border-none bg-sky-500/10",
        title: "Fetching ruleset…",
      };
    }
    return {
      text: "Pending",
      variant: "secondary",
      class: "text-muted-foreground",
      title: "Waiting for core status…",
    };
  }
</script>

<div class="w-full text-sm">
  <div class="sticky top-0 uppercase text-xs bg-background z-20 grid grid-cols-[32px_44px_80px_minmax(0,1fr)_100px_minmax(0,1fr)_120px_120px_minmax(0,1fr)_40px] px-2 border-b border-border/70">
    <div class="h-8 flex items-center justify-center">
      <Checkbox aria-label="Select all rulesets" {disabled}
        checked={ruleSets.length > 0 && selection.ids.length === ruleSets.length}
        indeterminate={selection.ids.length > 0 && selection.ids.length < ruleSets.length}
        onCheckedChange={(checked) => selection.selectAll(checked)} />
    </div>
    <div class="h-8 flex items-center font-medium text-muted-foreground"></div>
    <div class="h-8 flex items-center font-medium text-muted-foreground px-2">Type</div>
    <div class="h-8 flex items-center font-medium text-muted-foreground px-2">Name</div>
    <div class="h-8 flex items-center font-medium text-muted-foreground px-2">Status</div>
    <div class="h-8 flex items-center font-medium text-muted-foreground px-2">Source</div>
    <div class="h-8 flex items-center font-medium text-muted-foreground px-2">Download via</div>
    <div class="h-8 flex items-center font-medium text-muted-foreground px-2">Action</div>
    <div class="h-8 flex items-center font-medium text-muted-foreground px-2">Comment</div>
    <div class="sr-only">Actions</div>
  </div>
  <div class="p-0">
    {#if ruleSets.length === 0}
      <div class="p-6">
        <Empty.Root class="border border-dashed border-border/50 bg-card/20 py-12">
          <Empty.Media variant="icon">
            <HugeiconsIcon icon={Layers01Icon} class="size-5 text-muted-foreground" />
          </Empty.Media>
          <Empty.Header>
            <Empty.Title>No rulesets yet</Empty.Title>
            <Empty.Description>
              Subscribe to remote rule sets or import local files for reusable routing lists.
            </Empty.Description>
          </Empty.Header>
          {#if onAddRuleSet}
            <Empty.Content>
              <Button variant="outline" size="sm" class="gap-1.5" onclick={onAddRuleSet}>
                <HugeiconsIcon icon={Add01Icon} class="size-4" />
                Add Ruleset
              </Button>
            </Empty.Content>
          {/if}
        </Empty.Root>
      </div>
    {:else}
      {#each ruleSets as rs (rs.id)}
        {@const st = statusOf(rs)}
        <DnsRowActions
          label={`ruleset ${rs.name || rs.tag}`}
          selected={selection.ids.includes(rs.id)}
          {disabled}
          class="h-11 grid grid-cols-[32px_44px_80px_minmax(0,1fr)_100px_minmax(0,1fr)_120px_120px_minmax(0,1fr)_40px] items-center rounded-xl hover:bg-muted/30 data-[state=selected]:bg-muted/40 transition-colors cursor-pointer {!rs.enabled ? 'opacity-50' : ''}"
          onSelect={(event) => { onSelect(rs.id); selection.select(rs.id, event); }}
          onContextSelect={() => { onSelect(rs.id); selection.select(rs.id); }}
          onEdit={() => onEdit(rs.id)}
          onDelete={() => onDelete(rs.id)}
          enabled={rs.enabled}
          onToggleEnabled={() => onToggleEnabled?.(rs.id, !rs.enabled)}
        >
          <div class="flex items-center justify-center">
            <Checkbox aria-label={`Select ruleset ${rs.name || rs.tag}`} {disabled}
              checked={selection.ids.includes(rs.id)} onCheckedChange={(checked) => selection.toggle(rs.id, checked)} />
          </div>
          <div class="px-2 flex items-center justify-center">
            <CellEnabled
              checked={rs.enabled}
              {disabled}
              onChange={(v) => onToggleEnabled?.(rs.id, v)}
            />
          </div>
          <div class="px-2 flex items-center">
            <Badge variant="secondary" class="text-muted-foreground px-1.5 uppercase">
              {rs.type}
            </Badge>
          </div>
          <div class="px-2 truncate font-medium text-foreground" title={rs.name || rs.tag}>
            {rs.name || rs.tag}
          </div>
          <div class="px-2 flex items-center">
            <Badge variant={st.variant} class={st.class} title={st.title}>
              {st.text}
            </Badge>
          </div>
          <div class="px-2 truncate" title={rs.source}>
            {rs.type === "local" ? rs.source.split("/").pop() : rs.source}
          </div>
          <div
            class="px-2 text-sm text-muted-foreground truncate"
            title={rs.type === "remote" ? rs.download_policy?.trim() || "direct" : undefined}
          >
            {rs.type === "remote" ? downloadViaLabel(rs) : "—"}
          </div>
          <div class="px-2 text-sm truncate">
            {getActionName(rs.action)}
          </div>
          <div class="px-2 text-muted-foreground/60 italic truncate" title={rs.comment || ""}>
            {rs.comment || "—"}
          </div>
        </DnsRowActions>
        <div class="h-px bg-border/40 mx-2 last:hidden" aria-hidden="true"></div>
      {/each}
    {/if}
  </div>
</div>
