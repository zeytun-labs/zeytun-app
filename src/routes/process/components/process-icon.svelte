<script module>
  // Use a module-level cache so we don't re-fetch for every row
  // that uses the same process name
  const iconCache = new Map<string, string | null>();
</script>

<script lang="ts">
  import { HugeiconsIcon } from "@hugeicons/svelte";
  import { CommandLineIcon } from "@hugeicons/core-free-icons";
  import { getProcessIcon } from "$lib/core/api";
  import { convertFileSrc } from "@tauri-apps/api/core";
  import { onMount } from "svelte";

  let { name, path = "", class: className = "" } = $props<{ name?: string; path?: string; class?: string }>();

  let iconSrc = $state<string | null>(null);

  $effect(() => {
    const procName = name || (path ? path.split('/').pop() : null);
    if (!procName) return;
    
    if (iconCache.has(procName)) {
      iconSrc = iconCache.get(procName) || null;
      return;
    }

    getProcessIcon(procName, path)
      .then((absPath) => {
        if (absPath) {
          const src = convertFileSrc(absPath);
          iconCache.set(procName, src);
          iconSrc = src;
        }
      })
      .catch(() => {
        iconCache.set(procName, null);
        iconSrc = null;
      });
  });
</script>

{#if iconSrc}
  <img src={iconSrc} alt="" class="object-contain {className}" />
{:else}
  <HugeiconsIcon
    icon={CommandLineIcon}
    class="text-muted-foreground/60 {className}"
  />
{/if}
