<script lang="ts">
  import * as Dialog from "$lib/components/ui/dialog/index.js";
  import { Button } from "$lib/components/ui/button/index.js";
  import { Input } from "$lib/components/ui/input/index.js";
  import { Label } from "$lib/components/ui/label/index.js";
  import * as Select from "$lib/components/ui/select/index.js";
  import * as RadioGroup from "$lib/components/ui/radio-group/index.js";
  import type { ActionGroup } from "./rule-dialog.svelte";
  import type { RuleSet } from "$lib/core/types";
  import { coreStageRulesetFile } from "$lib/core/api";
  import { errorMessage } from "$lib/errors";

  export type RulesetDraftPayload = {
    type: "local" | "remote";
    source: string;
    action: string;
    comment: string | null;
    download_policy?: string | null;
    name?: string | null;
  };

  interface Props {
    open: boolean;
    mode?: "create" | "edit";
    ruleset?: RuleSet | null;
    actionGroups: ActionGroup[];
    getActionName: (tag: string) => string;
    onSave: (data: RulesetDraftPayload) => Promise<void>;
  }

  let {
    open = $bindable(false),
    mode = "create",
    ruleset = null,
    actionGroups,
    getActionName,
    onSave,
  }: Props = $props();

  let sourceType = $state<"local" | "remote">("local");
  let url = $state("");
  let file = $state<File | null>(null);
  let localPath = $state(""); // kept when editing local without re-upload
  let actionStr = $state("");
  let comment = $state("");
  let rulesetName = $state("");
  let saving = $state(false);
  let error = $state<string | null>(null);

  $effect(() => {
    if (!open) return;
    error = null;
    file = null;
    if (mode === "edit" && ruleset) {
      sourceType = ruleset.type === "remote" ? "remote" : "local";
      url = ruleset.type === "remote" ? ruleset.source : "";
      localPath = ruleset.type === "local" ? ruleset.source : "";
      actionStr = ruleset.action;
      comment = ruleset.comment ?? "";
      rulesetName = ruleset.name ?? "";
    } else {
      sourceType = "local";
      url = "";
      localPath = "";
      actionStr = "";
      comment = "";
      rulesetName = "";
    }
  });

  function handleFileChange(e: Event) {
    const target = e.target as HTMLInputElement;
    file = target.files?.[0] ?? null;
  }

  const isValidUrl = (s: string) => {
    try {
      new URL(s);
      return true;
    } catch {
      return false;
    }
  };

  const isFormValid = $derived.by(() => {
    if (!actionStr) return false;
    if (!rulesetName.trim()) return false;
    if (sourceType === "remote") return isValidUrl(url);
    // local: new file or existing path (edit)
    return file !== null || (mode === "edit" && !!localPath);
  });

  async function handleSubmit(e: Event) {
    e.preventDefault();
    if (!isFormValid) return;

    saving = true;
    error = null;
    try {
      let source: string;
      if (sourceType === "remote") {
        source = url.trim();
      } else if (file) {
        if (!file.name.endsWith(".srs")) {
          error = "Only .srs files are supported";
          return;
        }
        const bytes = Array.from(new Uint8Array(await file.arrayBuffer()));
        source = await coreStageRulesetFile(bytes);
      } else {
        source = localPath;
      }

      await onSave({
        type: sourceType,
        source,
        action: actionStr,
        comment: comment.trim() || null,
        download_policy:
          mode === "edit" && ruleset?.type === "remote"
            ? ruleset.download_policy
            : null,
        name: rulesetName.trim() || null,
      });
      open = false;
    } catch (err) {
      error = errorMessage(err);
    } finally {
      saving = false;
    }
  }
</script>

<Dialog.Root bind:open>
  <Dialog.Content class="sm:max-w-md">
    <Dialog.Header>
      <Dialog.Title>{mode === "edit" ? "Edit Ruleset" : "Add Ruleset"}</Dialog.Title>
      <Dialog.Description>
        {mode === "edit"
          ? "Changes stay draft until you Publish."
          : "Import a standard Sing-box binary ruleset (.srs). Draft until Publish."}
      </Dialog.Description>
    </Dialog.Header>

    {#if error}
      <div class="text-sm text-red-500 bg-red-500/10 p-2 rounded">
        {error}
      </div>
    {/if}

    <form id="ruleset-form" class="space-y-6 pt-2" onsubmit={handleSubmit}>
      <div class="space-y-2">
        <Label for="rs-name">Name</Label>
        <Input
          id="rs-name"
          bind:value={rulesetName}
          placeholder="e.g. Iran Sites, Ad Block, Bypass List"
        />
      </div>

      <div class="space-y-3">
        <Label>Source Type</Label>
        <RadioGroup.Root bind:value={sourceType} class="flex gap-4">
          <div class="flex items-center space-x-2">
            <RadioGroup.Item value="local" id="rs-local" />
            <Label for="rs-local" class="font-normal cursor-pointer">Local File</Label>
          </div>
          <div class="flex items-center space-x-2">
            <RadioGroup.Item value="remote" id="rs-remote" />
            <Label for="rs-remote" class="font-normal cursor-pointer">Remote URL</Label>
          </div>
        </RadioGroup.Root>
      </div>

      {#if sourceType === "local"}
        <div class="space-y-2">
          <Label for="srs-file">File (.srs)</Label>
          <Input id="srs-file" type="file" accept=".srs" onchange={handleFileChange} />
          {#if mode === "edit" && localPath && !file}
            <p class="text-muted-foreground truncate text-xs" title={localPath}>
              Current: {localPath.split("/").pop()}
            </p>
          {/if}
        </div>
      {:else}
        <div class="space-y-2">
          <Label for="srs-url">URL</Label>
          <Input
            id="srs-url"
            type="url"
            placeholder="https://example.com/rules.srs"
            bind:value={url}
          />
        </div>
      {/if}

      <div class="space-y-2">
        <Label>Action</Label>
        <Select.Root type="single" name="action" bind:value={actionStr}>
          <Select.Trigger class="w-full">
            {actionStr ? getActionName(actionStr) : "Select action..."}
          </Select.Trigger>
          <Select.Content>
            {#each actionGroups as group (group.label || "default")}
              {#if group.options.length > 0}
                <Select.Group>
                  {#if group.label}
                    <Select.Label>{group.label}</Select.Label>
                  {/if}
                  {#each group.options as opt (opt.tag)}
                    <Select.Item value={opt.tag} label={opt.name}>
                      {opt.name}
                    </Select.Item>
                  {/each}
                </Select.Group>
              {/if}
            {/each}
          </Select.Content>
        </Select.Root>
      </div>

      <div class="space-y-2">
        <Label for="rs-comment">Comment (Optional)</Label>
        <Input id="rs-comment" bind:value={comment} placeholder="Notes..." />
      </div>

      <Dialog.Footer>
        <Button type="button" variant="outline" onclick={() => (open = false)}>
          Cancel
        </Button>
        <Button type="submit" disabled={!isFormValid || saving}>
          {saving ? "Saving..." : mode === "edit" ? "Apply to draft" : "Add to draft"}
        </Button>
      </Dialog.Footer>
    </form>
  </Dialog.Content>
</Dialog.Root>
