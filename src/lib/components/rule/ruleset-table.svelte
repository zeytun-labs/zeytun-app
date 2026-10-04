<script lang="ts">
  import type { RuleSet } from "$lib/core/types";
  import { Badge, type BadgeVariant } from "$lib/components/ui/badge/index.js";
  import { notificationStore } from "$lib/stores/notifications.svelte";
  import CellEnabled from "./cell-enabled.svelte";

  interface Props {
    ruleSets: RuleSet[];
    selectedId: string | null;
    getActionName: (tag: string) => string;
    onSelect: (id: string) => void;
    onEdit: (id: string) => void;
    onToggleEnabled?: (id: string, enabled: boolean) => void;
  }

  let {
    ruleSets,
    selectedId,
    getActionName,
    onSelect,
    onEdit,
    onToggleEnabled,
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

{#if ruleSets.length === 0}
  <div class="text-muted-foreground p-8 text-center text-sm">No rulesets yet.</div>
{:else}
    <div class="w-full text-sm">
      <div class="sticky top-0 uppercase text-xs bg-background z-20 grid grid-cols-[44px_100px_1fr_120px_1fr_160px_160px_1fr] px-2 border-b border-border/70">
        <div class="h-8 flex items-center font-medium text-muted-foreground"></div>
        <div class="h-8 flex items-center font-medium text-muted-foreground px-2">Type</div>
        <div class="h-8 flex items-center font-medium text-muted-foreground px-2">Name</div>
        <div class="h-8 flex items-center font-medium text-muted-foreground px-2">Status</div>
        <div class="h-8 flex items-center font-medium text-muted-foreground px-2">Source</div>
        <div class="h-8 flex items-center font-medium text-muted-foreground px-2">Download via</div>
        <div class="h-8 flex items-center font-medium text-muted-foreground px-2">Action</div>
        <div class="h-8 flex items-center font-medium text-muted-foreground px-2">Comment</div>
      </div>
      <div class="p-0">
        {#each ruleSets as rs (rs.id)}
          {@const st = statusOf(rs)}
          <!-- svelte-ignore a11y_click_events_have_key_events -->
          <!-- svelte-ignore a11y_no_static_element_interactions -->
          <div
            data-state={selectedId === rs.id ? "selected" : undefined}
            class="h-11 grid grid-cols-[44px_100px_1fr_120px_1fr_160px_160px_1fr] items-center rounded-xl hover:bg-muted/30 data-[state=selected]:bg-muted/40 transition-colors cursor-pointer {!rs.enabled ? 'opacity-50' : ''}"
            onclick={() => onSelect(rs.id)}
            ondblclick={() => onEdit(rs.id)}
          >
            <div class="px-2 flex items-center justify-center">
              <CellEnabled
                checked={rs.enabled}
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
          </div>
          <div class="h-px bg-border/40 mx-2 last:hidden" aria-hidden="true"></div>
        {/each}
      </div>
    </div>
{/if}
