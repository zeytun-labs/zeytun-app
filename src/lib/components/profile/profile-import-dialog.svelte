<script lang="ts">
  import * as Dialog from "$lib/components/ui/dialog/index.js";
  import { Button } from "$lib/components/ui/button/index.js";
  import { Input } from "$lib/components/ui/input/index.js";
  import { Label } from "$lib/components/ui/label/index.js";
  import Spinner from "$lib/components/ui/spinner/spinner.svelte";
  import { coreProfileImportPreview, coreProfileImportConfirm } from "$lib/core/api";
  import { profileStore } from "$lib/stores/profile.svelte";
  import type { ImportPreview } from "$lib/core/types";
  import { FileUploadIcon } from "@hugeicons/core-free-icons";
  import { HugeiconsIcon } from "@hugeicons/svelte";
  import { toast } from "svelte-sonner";

  interface Props {
    open: boolean;
  }

  let { open = $bindable(false) }: Props = $props();

  let fileInputRef = $state<HTMLInputElement | null>(null);
  let rawContent = $state<string | null>(null);
  let preview = $state<ImportPreview | null>(null);
  let customName = $state("");
  let parsing = $state(false);
  let importing = $state(false);
  let error = $state<string | null>(null);

  $effect(() => {
    if (open) {
      rawContent = null;
      preview = null;
      customName = "";
      error = null;
    }
  });

  async function handleFileSelected(e: Event) {
    const input = e.target as HTMLInputElement;
    const file = input.files?.[0];
    if (!file) return;

    if (file.size > 32 * 1024 * 1024) {
      error = "File size exceeds maximum allowed 32 MiB";
      return;
    }

    parsing = true;
    error = null;
    try {
      const text = await file.text();
      rawContent = text;
      const prev = await coreProfileImportPreview(text);
      preview = prev;
      customName = prev.name;
    } catch (err: any) {
      error = `Invalid profile document: ${err?.message ?? err}`;
      preview = null;
      rawContent = null;
    } finally {
      parsing = false;
    }
  }

  async function handleConfirmImport() {
    if (!rawContent || !preview) return;
    importing = true;
    error = null;
    try {
      const created = await coreProfileImportConfirm(
        rawContent,
        preview.preview_token,
        customName.trim() || null,
      );
      await profileStore.load();
      toast.success(`Profile "${created.name}" imported successfully`);
      open = false;
    } catch (err: any) {
      error = `Import failed: ${err?.message ?? err}`;
    } finally {
      importing = false;
    }
  }
</script>

<Dialog.Root bind:open>
  <Dialog.Content class="sm:max-w-md">
    <Dialog.Header>
      <Dialog.Title>Import Native Profile</Dialog.Title>
      <Dialog.Description>
        Import a Zeytun profile document (.zeytun.json).
      </Dialog.Description>
    </Dialog.Header>

    <div class="flex flex-col gap-4 py-3">
      {#if error}
        <div class="rounded-xl border border-destructive/30 bg-destructive/10 p-3 text-xs text-destructive">
          {error}
        </div>
      {/if}

      {#if !preview}
        <input
          type="file"
          accept=".json,.zeytun.json"
          class="hidden"
          bind:this={fileInputRef}
          onchange={handleFileSelected}
        />

        <button
          type="button"
          class="flex flex-col items-center justify-center gap-2 rounded-2xl border border-dashed border-border/80 p-8 text-center transition-colors hover:border-primary/50 hover:bg-muted/10 cursor-pointer"
          onclick={() => fileInputRef?.click()}
          disabled={parsing}
        >
          <div class="flex size-10 items-center justify-center rounded-xl bg-muted text-muted-foreground">
            {#if parsing}
              <Spinner class="size-5" />
            {:else}
              <HugeiconsIcon icon={FileUploadIcon} class="size-5" />
            {/if}
          </div>
          <span class="text-sm font-medium">Choose a .zeytun.json file</span>
          <span class="text-xs text-muted-foreground">Click to browse your files</span>
        </button>
      {:else}
        <div class="flex flex-col gap-3">
          <div class="flex flex-col gap-1.5">
            <Label for="import-profile-name">Profile Name</Label>
            <Input id="import-profile-name" bind:value={customName} placeholder={preview.name} />
          </div>

          <div class="grid grid-cols-2 gap-2 rounded-xl bg-background/50 border border-border/40 p-3 text-xs">
            <div>
              <span class="text-muted-foreground">Type:</span>
              <span class="ml-1 font-medium capitalize">{preview.mode}</span>
            </div>
            <div>
              <span class="text-muted-foreground">Sources:</span>
              <span class="ml-1 font-medium">{preview.source_count}</span>
            </div>
            <div>
              <span class="text-muted-foreground">Proxies:</span>
              <span class="ml-1 font-medium">{preview.proxy_count}</span>
            </div>
            <div>
              <span class="text-muted-foreground">Policies:</span>
              <span class="ml-1 font-medium">{preview.policy_count}</span>
            </div>
            <div>
              <span class="text-muted-foreground">Rules:</span>
              <span class="ml-1 font-medium">{preview.rule_count}</span>
            </div>
            {#if preview.asset_count > 0}
              <div>
                <span class="text-muted-foreground">Assets:</span>
                <span class="ml-1 font-medium">{preview.asset_count}</span>
              </div>
            {/if}
          </div>

          {#if preview.warnings.length > 0}
            <div class="rounded-xl border border-amber-500/30 bg-amber-500/10 p-2.5 text-[11px] text-amber-500">
              {#each preview.warnings as warn}
                <p>{warn}</p>
              {/each}
            </div>
          {/if}
        </div>
      {/if}
    </div>

    <Dialog.Footer class="gap-2">
      <Button
        type="button"
        variant="outline"
        onclick={() => (open = false)}
        disabled={importing}
      >
        Cancel
      </Button>
      {#if preview}
        <Button onclick={handleConfirmImport} disabled={importing}>
          {#if importing}
            <Spinner class="mr-2 size-4 border-white/30 border-t-white" />
          {/if}
          Import as New Profile
        </Button>
      {/if}
    </Dialog.Footer>
  </Dialog.Content>
</Dialog.Root>
