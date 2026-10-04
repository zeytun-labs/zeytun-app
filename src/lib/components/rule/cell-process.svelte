<script module>
  import type { ResolvedProcess } from "$lib/core/types";

  // Module-level cache so identical rule values resolve once per session.
  const resolveCache = new Map<string, ResolvedProcess | null>();

  // Rule kinds whose value is a process name/path/regex. Exported so the table
  // can skip its native `title` tooltip for these rows (we render our own).
  export const PROCESS_KINDS = new Set([
    "PROCESS-NAME",
    "PROCESS-PATH",
    "PROCESS-PATH-REGEX",
  ]);
</script>

<script lang="ts">
  import { HugeiconsIcon } from "@hugeicons/svelte";
  import { CommandLineIcon } from "@hugeicons/core-free-icons";
  import * as Tooltip from "$lib/components/ui/tooltip";
  import { convertFileSrc } from "@tauri-apps/api/core";
  import { coreResolveProcess } from "$lib/core/api";

  let { kind, value }: { kind: string; value: string } = $props();

  let resolved = $state<ResolvedProcess | null>(null);

  $effect(() => {
    const key = `${kind}|${value}`;
    if (resolveCache.has(key)) {
      resolved = resolveCache.get(key) ?? null;
      return;
    }
    let cancelled = false;
    coreResolveProcess(kind, value)
      .then((r) => {
        if (cancelled) return;
        resolveCache.set(key, r ?? null);
        resolved = r ?? null;
      })
      .catch(() => {
        if (cancelled) return;
        resolveCache.set(key, null);
      });
    return () => {
      cancelled = true;
    };
  });
</script>

<Tooltip.Provider>
  <Tooltip.Root>
    <Tooltip.Trigger
      class="flex w-full min-w-0 cursor-default items-center gap-1.5 text-left"
    >
      {#if resolved?.iconPath}
        <img
          src={convertFileSrc(resolved.iconPath)}
          alt=""
          class="size-4 shrink-0 rounded-[4px] object-contain"
        />
      {:else}
        <HugeiconsIcon
          icon={CommandLineIcon}
          class="size-4 shrink-0 text-muted-foreground/60"
        />
      {/if}
      <span class="truncate"
        >{resolved?.name ?? (kind === "PROCESS-NAME" ? value : "…")}</span
      >
    </Tooltip.Trigger>
    <Tooltip.Content side="top" sideOffset={6}>
      <span class="max-w-[240px] break-all font-mono">{value}</span>
    </Tooltip.Content>
  </Tooltip.Root>
</Tooltip.Provider>
