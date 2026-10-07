<script lang="ts">
  import * as Dialog from "$lib/components/ui/dialog/index.js";
  import * as RadioGroup from "$lib/components/ui/radio-group/index.js";
  import { Label } from "$lib/components/ui/label/index.js";
  import { Button } from "$lib/components/ui/button/index.js";
  import Spinner from "$lib/components/ui/spinner/spinner.svelte";
  import { coreProfileExport } from "$lib/core/api";
  import type { ExportMode, ProfileMeta } from "$lib/core/types";
  import { downloadDir, join } from "@tauri-apps/api/path";
  import { revealItemInDir } from "@tauri-apps/plugin-opener";
  import { toast } from "svelte-sonner";

  interface Props {
    open: boolean;
    profile: ProfileMeta | null;
  }

  let { open = $bindable(false), profile }: Props = $props();

  let mode = $state<ExportMode>("full");
  let exporting = $state(false);

  async function handleExport() {
    if (!profile) return;
    exporting = true;
    try {
      const jsonStr = await coreProfileExport(profile.id, mode);
      const blob = new Blob([jsonStr], { type: "application/json" });
      const url = URL.createObjectURL(blob);
      const a = document.createElement("a");
      const safeName = profile.name.toLowerCase().replace(/[^a-z0-9_-]/g, "-");
      const suffix = mode === "snapshot" ? ".snapshot" : "";
      const filename = `${safeName}${suffix}.zeytun.json`;
      a.href = url;
      a.download = filename;
      document.body.appendChild(a);
      a.click();
      document.body.removeChild(a);
      URL.revokeObjectURL(url);

      try {
        const dlDir = await downloadDir();
        const fullPath = await join(dlDir, filename);
        toast.success(`Exported to Downloads: ${filename}`, {
          action: {
            label: "Show in Finder",
            onClick: () => {
              void revealItemInDir(fullPath).catch(() => {});
            },
          },
        });
      } catch {
        toast.success(`Exported to Downloads: ${filename}`);
      }

      open = false;
    } catch (err: any) {
      toast.error(`Export failed: ${err?.message ?? err}`);
    } finally {
      exporting = false;
    }
  }
</script>

<Dialog.Root bind:open>
  <Dialog.Content class="sm:max-w-md">
    <Dialog.Header>
      <Dialog.Title>Export Profile</Dialog.Title>
      <Dialog.Description>
        Export "{profile?.name}" to your Downloads folder as a portable native document (.zeytun.json).
      </Dialog.Description>
    </Dialog.Header>

    <RadioGroup.Root bind:value={mode} class="flex flex-col gap-3 py-4">
      <label
        for="export-mode-full"
        class="flex items-start gap-3 rounded-2xl border border-border/40 p-3.5 cursor-pointer hover:bg-muted/20 hover:border-border transition-colors has-data-[state=checked]:border-primary/40 has-data-[state=checked]:bg-primary/5"
      >
        <RadioGroup.Item
          value="full"
          id="export-mode-full"
          class="mt-0.5"
        />
        <div class="flex flex-col gap-0.5">
          <Label for="export-mode-full" class="font-medium text-sm text-foreground cursor-pointer">
            Full Profile (Recommended for transfer)
          </Label>
          <span class="text-muted-foreground text-xs leading-relaxed">
            Includes subscription URLs and all proxy definitions. Allows continuing sync on another device.
          </span>
          <span class="text-[11px] text-amber-500 font-medium mt-1">
            ⚠️ Contains unencrypted private subscription URLs and credentials.
          </span>
        </div>
      </label>

      <label
        for="export-mode-snapshot"
        class="flex items-start gap-3 rounded-2xl border border-border/40 p-3.5 cursor-pointer hover:bg-muted/20 hover:border-border transition-colors has-data-[state=checked]:border-primary/40 has-data-[state=checked]:bg-primary/5"
      >
        <RadioGroup.Item
          value="snapshot"
          id="export-mode-snapshot"
          class="mt-0.5"
        />
        <div class="flex flex-col gap-0.5">
          <Label for="export-mode-snapshot" class="font-medium text-sm text-foreground cursor-pointer">
            Offline Snapshot
          </Label>
          <span class="text-muted-foreground text-xs leading-relaxed">
            Converts all subscription proxies to independent Manual nodes and completely removes subscription URLs.
          </span>
        </div>
      </label>
    </RadioGroup.Root>

    <Dialog.Footer>
      <Button type="button" variant="outline" onclick={() => (open = false)} disabled={exporting}>
        Cancel
      </Button>
      <Button onclick={handleExport} disabled={exporting}>
        {#if exporting}
          <Spinner class="mr-2 size-4 border-white/30 border-t-white" />
        {/if}
        Export Document
      </Button>
    </Dialog.Footer>
  </Dialog.Content>
</Dialog.Root>
