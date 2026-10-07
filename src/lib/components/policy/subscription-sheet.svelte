<script lang="ts">
  import * as Sheet from "$lib/components/ui/sheet/index.js";
  import { Button } from "$lib/components/ui/button/index.js";
  import { Input } from "$lib/components/ui/input/index.js";
  import { Label } from "$lib/components/ui/label/index.js";
  import { Switch } from "$lib/components/ui/switch/index.js";
  import { ScrollArea } from "$lib/components/ui/scroll-area/index.js";
  import Spinner from "$lib/components/ui/spinner/spinner.svelte";
  import type { SubscriptionSource, SubscriptionSettings } from "$lib/core/types";
  import {
    ViewIcon,
    ViewOffIcon,
    Copy01Icon,
    Tick01Icon,
  } from "@hugeicons/core-free-icons";
  import { HugeiconsIcon } from "@hugeicons/svelte";
  import { toast } from "svelte-sonner";

  interface Props {
    open: boolean;
    mode?: "create" | "edit";
    source?: SubscriptionSource | null;
    saving: boolean;
    onSave: (settings: SubscriptionSettings) => Promise<void> | void;
  }

  let {
    open = $bindable(false),
    mode = "create",
    source = null,
    saving,
    onSave,
  }: Props = $props();

  let name = $state("");
  let url = $state("");
  let autoUpdate = $state(true);
  let updateIntervalHours = $state(24);
  let showUrl = $state(false);
  let copied = $state(false);
  let error = $state<string | null>(null);

  $effect(() => {
    if (open) {
      if (mode === "edit" && source) {
        name = source.settings.name;
        url = source.settings.url;
        autoUpdate = source.settings.auto_update;
        updateIntervalHours = source.settings.update_interval_hours;
      } else {
        name = "";
        url = "";
        autoUpdate = true;
        updateIntervalHours = 24;
      }
      showUrl = false;
      copied = false;
      error = null;
    }
  });

  async function handleCopy() {
    if (!url) return;
    try {
      await navigator.clipboard.writeText(url);
      copied = true;
      toast.success("URL copied to clipboard");
      setTimeout(() => (copied = false), 2000);
    } catch {
      toast.error("Failed to copy URL");
    }
  }

  async function handleSubmit(e: SubmitEvent) {
    e.preventDefault();
    error = null;

    const trimmedName = name.trim();
    const trimmedUrl = url.trim();

    if (!trimmedName) {
      error = "Name is required";
      return;
    }
    if (!trimmedUrl) {
      error = "URL is required";
      return;
    }
    if (!trimmedUrl.startsWith("http://") && !trimmedUrl.startsWith("https://")) {
      error = "URL must start with http:// or https://";
      return;
    }

    try {
      await onSave({
        name: trimmedName,
        url: trimmedUrl,
        enabled: true,
        auto_update: autoUpdate,
        update_interval_hours: updateIntervalHours,
      });
      open = false;
    } catch (err: any) {
      error = err?.message ?? String(err);
    }
  }
</script>

<Sheet.Root bind:open>
  <Sheet.Content
    side="right"
    class="flex flex-col gap-0 px-1 transition-all duration-300 w-full sm:max-w-md"
  >
    <form
      id="subscription-form"
      class="flex flex-1 flex-col h-full overflow-hidden"
      onsubmit={handleSubmit}
    >
      <ScrollArea class="h-0 flex-1 min-h-0">
        <Sheet.Header class="px-3">
          <Sheet.Title>
            {mode === "create" ? "Add Subscription" : "Edit Subscription"}
          </Sheet.Title>
          <Sheet.Description>
            {mode === "create"
              ? "Add an upstream subscription URL to automatically sync proxy nodes."
              : "Update upstream subscription settings."}
          </Sheet.Description>
        </Sheet.Header>

        <div class="flex flex-col gap-3 px-3">
          {#if error}
            <div class="rounded-xl border border-destructive/30 bg-destructive/10 p-3 text-xs text-destructive">
              {error}
            </div>
          {/if}

          <div class="flex flex-col gap-2">
            <Label for="sub-name">Name</Label>
            <Input
              id="sub-name"
              bind:value={name}
              placeholder="e.g. Primary VPN"
              required
              disabled={saving}
            />
          </div>

          <div class="flex flex-col gap-2">
            <div class="flex items-center justify-between">
              <Label for="sub-url">Subscription URL</Label>
              <div class="flex items-center gap-1">
                {#if url}
                  <Button
                    type="button"
                    variant="ghost"
                    size="icon"
                    class="size-7"
                    onclick={handleCopy}
                    title="Copy URL"
                  >
                    <HugeiconsIcon icon={copied ? Tick01Icon : Copy01Icon} class="size-3.5" />
                  </Button>
                  <Button
                    type="button"
                    variant="ghost"
                    size="icon"
                    class="size-7"
                    onclick={() => (showUrl = !showUrl)}
                    title={showUrl ? "Mask URL" : "Reveal URL"}
                  >
                    <HugeiconsIcon icon={showUrl ? ViewOffIcon : ViewIcon} class="size-3.5" />
                  </Button>
                {/if}
              </div>
            </div>
            <Input
              id="sub-url"
              bind:value={url}
              type={showUrl ? "text" : "password"}
              placeholder="https://..."
              required
              disabled={saving}
            />
          </div>

          <div class="flex items-center justify-between py-1">
            <div class="flex flex-col gap-0.5">
              <Label for="sub-auto-update" class="cursor-pointer text-sm font-medium">Automatic Updates</Label>
              <span class="text-xs text-muted-foreground">Background periodic synchronization</span>
            </div>
            <Switch id="sub-auto-update" bind:checked={autoUpdate} disabled={saving} />
          </div>

          <div class="flex flex-col gap-2">
            <Label for="sub-interval">Update Interval (hours)</Label>
            <Input
              id="sub-interval"
              type="number"
              min="1"
              max="720"
              bind:value={updateIntervalHours}
              disabled={saving || !autoUpdate}
            />
          </div>

          <div class="h-4"></div>
        </div>
      </ScrollArea>

      <Sheet.Footer
        class="-mx-1 bg-background border-t border-border flex-row justify-end"
      >
        <Button
          type="button"
          variant="outline"
          onclick={() => (open = false)}
          disabled={saving}
        >
          Cancel
        </Button>
        <Button type="submit" class="flex-1" disabled={saving}>
          {#if saving}
            <Spinner class="mr-2 size-4 border-white/30 border-t-white" />
          {/if}
          {saving ? "Saving..." : "Save"}
        </Button>
      </Sheet.Footer>
    </form>
  </Sheet.Content>
</Sheet.Root>
