<script lang="ts">
  import { createActionQueue } from "$lib/composables/action-queue";
  import { useRowSelection } from "$lib/composables/row-selection.svelte";
  import SelectionRegion from "$lib/components/rule/selection-region.svelte";
  import RuleSelectionToolbar from "$lib/components/rule/rule-selection-toolbar.svelte";
  import DnsRowActions from "$lib/components/rule/dns-row-actions.svelte";
  import { Checkbox } from "$lib/components/ui/checkbox";
  import { toast } from "svelte-sonner";
  import * as Card from "$lib/components/ui/card";
  import * as Empty from "$lib/components/ui/empty/index.js";
  import * as Dialog from "$lib/components/ui/dialog/index.js";
  import { Button } from "$lib/components/ui/button/index.js";
  import { Input } from "$lib/components/ui/input/index.js";
  import { Label } from "$lib/components/ui/label/index.js";
  import { Switch } from "$lib/components/ui/switch";
  import { HugeiconsIcon } from "@hugeicons/svelte";
  import CellEnabled from "$lib/components/rule/cell-enabled.svelte";
  import {
    Add01Icon,
    ComputerIcon,
    Delete01Icon,
    Edit01Icon,
  } from "@hugeicons/core-free-icons";
  import { coreUpdateDnsHosts } from "$lib/core/api";
  import type { DnsHostEntry } from "$lib/core/types";
  import { profileStore } from "$lib/stores/profile.svelte";
  import { errorMessage } from "$lib/errors";

  let { triggerAdd = $bindable() } = $props<{ triggerAdd?: () => void }>();
  const writes = createActionQueue(() => profileStore.profile?.id);

  let isSaving = $state(false);
  let operationError = $state<string | null>(null);
  let dialogOpen = $state(false);
  let dialogMode = $state<"create" | "edit">("create");
  let editingId = $state<string | null>(null);
  let domain = $state("");
  let address = $state("");
  let dialogError = $state<string | null>(null);

  const hosts = $derived<DnsHostEntry[]>(
    profileStore.profile?.dns?.hosts ?? [],
  );

  const selection = useRowSelection(() => hosts.map((h) => h.id), () => profileStore.profile?.id);
  const selectedHosts = $derived(hosts.filter((h) => selection.ids.includes(h.id)));
  $effect(() => { triggerAdd = openCreateDialog; });

  function normalizeDomain(value: string): string {
    return value.trim().toLowerCase().replace(/\.$/, "");
  }

  function isValidDomain(value: string): boolean {
    if (value.length === 0 || value.length > 253 || value.includes("..")) {
      return false;
    }

    return value.split(".").every(
      (label) =>
        label.length > 0 &&
        label.length <= 63 &&
        /^[a-z0-9](?:[a-z0-9-]*[a-z0-9])?$/i.test(label),
    );
  }

  function isValidIpv4(value: string): boolean {
    const parts = value.split(".");
    return (
      parts.length === 4 &&
      parts.every(
        (part) =>
          /^\d{1,3}$/.test(part) &&
          Number(part) >= 0 &&
          Number(part) <= 255,
      )
    );
  }

  function isValidIpv6(value: string): boolean {
    if (!value.includes(":") || !/^[0-9a-f:]+$/i.test(value)) return false;

    const halves = value.split("::");
    if (halves.length > 2) return false;

    const groups = halves.flatMap((half) =>
      half === "" ? [] : half.split(":"),
    );
    if (!groups.every((group) => /^[0-9a-f]{1,4}$/i.test(group))) {
      return false;
    }

    return halves.length === 2 ? groups.length < 8 : groups.length === 8;
  }

  function openCreateDialog() {
    dialogMode = "create";
    editingId = null;
    domain = "";
    address = "";
    dialogError = null;
    dialogOpen = true;
  }

  function openEditDialog(host: DnsHostEntry) {
    dialogMode = "edit";
    editingId = host.id;
    domain = host.domain;
    address = host.address;
    dialogError = null;
    dialogOpen = true;
  }

  async function persistHosts(next: DnsHostEntry[]) {
    const profile = profileStore.profile;
    if (!profile) throw new Error("No active profile is available.");

    await coreUpdateDnsHosts(next);
    await profileStore.refresh();
  }

  async function saveHost(event: SubmitEvent) {
    event.preventDefault();
    if (isSaving) return;
    await writes.run(() => saveHostDraft(event));
  }

  async function saveHostDraft(event: SubmitEvent) {
    event.preventDefault();
    if (isSaving) return;

    const nextDomain = normalizeDomain(domain);
    const nextAddress = address.trim();
    dialogError = null;

    if (!isValidDomain(nextDomain)) {
      dialogError = "Enter a valid domain name, such as router.lan or example.com.";
      return;
    }
    if (!isValidIpv4(nextAddress) && !isValidIpv6(nextAddress)) {
      dialogError = "Enter a valid IPv4 or IPv6 address.";
      return;
    }
    if (
      hosts.some(
        (host) =>
          host.id !== editingId && normalizeDomain(host.domain) === nextDomain,
      )
    ) {
      dialogError = "A Local Hosts entry already exists for this domain.";
      return;
    }

    isSaving = true;
    try {
      let next: DnsHostEntry[];
      if (dialogMode === "create") {
        const host: DnsHostEntry = {
          id: crypto.randomUUID(),
          domain: nextDomain,
          address: nextAddress,
          enabled: true,
        };
        next = [host, ...hosts];
      } else {
        if (!editingId || !hosts.some((host) => host.id === editingId)) {
          throw new Error("The Local Hosts entry is no longer available.");
        }
        next = hosts.map((host) =>
          host.id === editingId
            ? { ...host, domain: nextDomain, address: nextAddress }
            : host,
        );
      }

      await persistHosts(next);
      operationError = null;
      dialogOpen = false;
      toast.success(
        dialogMode === "create"
          ? "Local Hosts entry added."
          : "Local Hosts entry updated.",
      );
    } catch (error) {
      console.error("Failed to save Local Hosts entry:", error);
      dialogError = `Could not save the Local Hosts entry: ${errorMessage(error)}`;
    } finally {
      isSaving = false;
    }
  }

  async function deleteHosts(ids: string[]) {
    if (isSaving) return;
    isSaving = true;
    operationError = null;
    try {
      await writes.run(() => persistHosts(hosts.filter((host) => !ids.includes(host.id))));
      selection.replace(selection.ids.filter((id) => !ids.includes(id)));
    } catch (error) {
      operationError = `Could not delete Local Hosts entries: ${errorMessage(error)}`;
      toast.error(operationError);
    } finally { isSaving = false; }
  }

  async function setHostsEnabled(ids: string[], enabled: boolean) {
    if (isSaving) return;
    operationError = null;
    try {
      await writes.run(() => persistHosts(hosts.map((host) => ids.includes(host.id) ? { ...host, enabled } : host)));
    } catch (error) {
      operationError = `Could not update Local Hosts entries: ${errorMessage(error)}`;
      toast.error(operationError);
    }
  }


</script>

<div class="flex flex-col gap-3 w-full text-sm">

  {#if operationError}
    <div class="mb-3 rounded-xl border border-destructive/30 bg-destructive/10 p-3 text-sm text-destructive">
      {operationError}
    </div>
  {/if}

  {#if hosts.length === 0}
    <div class="mt-2">
      <Empty.Root class="border border-dashed border-border/50 bg-card/20 py-12">
        <Empty.Media variant="icon">
          <HugeiconsIcon icon={ComputerIcon} class="size-5 text-muted-foreground" />
        </Empty.Media>
        <Empty.Header>
          <Empty.Title>No local host overrides</Empty.Title>
          <Empty.Description>
            Add a domain and IPv4 or IPv6 address to create a static DNS mapping.
          </Empty.Description>
        </Empty.Header>
        <Empty.Content>
          <Button
            variant="outline"
            size="sm"
            class="gap-1.5"
            onclick={openCreateDialog}
            disabled={isSaving || profileStore.loading}
          >
            <HugeiconsIcon icon={Add01Icon} class="size-4" />
            Add Local Host
          </Button>
        </Empty.Content>
      </Empty.Root>
    </div>
  {:else}
    <div>
      <div
        class="sticky top-0 uppercase text-xs bg-background z-20 grid grid-cols-[32px_60px_1fr_1fr_40px] px-2 border-b border-border/70"
      >
        <div class="h-8 flex items-center justify-center">
          <Checkbox aria-label="Select all hosts" disabled={isSaving}
            checked={hosts.length > 0 && selection.ids.length === hosts.length}
            indeterminate={selection.ids.length > 0 && selection.ids.length < hosts.length}
            onCheckedChange={(checked) => selection.selectAll(checked)} />
        </div>
        <div class="h-8 flex items-center font-medium text-muted-foreground px-2"></div>
        <div class="h-8 flex items-center font-medium text-muted-foreground px-2">Domain</div>
        <div class="h-8 flex items-center font-medium text-muted-foreground px-2">Address</div>
        <div class="sr-only">Actions</div>
      </div>
      <div class="flex flex-col">
        {#each hosts as host, i (host.id)}
          {#if i > 0}<div class="h-px w-full bg-border/40"></div>{/if}
          <DnsRowActions label={`host ${host.domain}`} selected={selection.ids.includes(host.id)} disabled={isSaving}
            class="h-12 grid grid-cols-[32px_60px_1fr_1fr_40px] items-center rounded-xl hover:bg-muted/30 data-[state=selected]:bg-muted/40 transition-colors px-2 cursor-pointer {host.enabled ? '' : 'opacity-50'}"
            onSelect={(event) => selection.select(host.id, event)} onContextSelect={() => selection.select(host.id)}
            onEdit={() => openEditDialog(host)} onDelete={() => deleteHosts([host.id])}
            enabled={host.enabled} onToggleEnabled={() => setHostsEnabled([host.id], !host.enabled)}
          >
            <div class="flex items-center justify-center">
              <Checkbox aria-label={`Select host ${host.domain}`} disabled={isSaving}
                checked={selection.ids.includes(host.id)} onCheckedChange={(checked) => selection.toggle(host.id, checked)} />
            </div>
            <div class="px-2 flex items-center justify-center">
              <CellEnabled
                checked={host.enabled}
                disabled={isSaving}
                onChange={(enabled) => setHostsEnabled([host.id], enabled)}
              />
            </div>
            <div class="px-2 truncate font-mono font-medium text-foreground" title={host.domain}>
              {host.domain}
            </div>
            <div class="px-2 truncate font-mono text-muted-foreground" title={host.address}>
              {host.address}
            </div>
          </DnsRowActions>
        {/each}
      </div>
    </div>
  {/if}
  <SelectionRegion open={selectedHosts.length > 0}>
    <RuleSelectionToolbar selectedCount={selectedHosts.length} itemLabel="host" saving={isSaving}
      canEnable={selectedHosts.some((h) => !h.enabled)} canDisable={selectedHosts.some((h) => h.enabled)}
      onEnable={() => setHostsEnabled(selection.ids, true)} onDisable={() => setHostsEnabled(selection.ids, false)}
      onDelete={() => deleteHosts(selection.ids)} />
  </SelectionRegion>
</div>

<Dialog.Root bind:open={dialogOpen}>
  <Dialog.Content class="flex flex-col sm:max-w-md">
    <form class="flex flex-col gap-4" onsubmit={saveHost}>
      <Dialog.Header>
        <Dialog.Title>
          {dialogMode === "create" ? "Add Local Host" : "Edit Local Host"}
        </Dialog.Title>
        <Dialog.Description>
          Map one domain to a static IPv4 or IPv6 address.
        </Dialog.Description>
      </Dialog.Header>

      {#if dialogError}
        <div class="rounded-xl border border-destructive/30 bg-destructive/10 p-3 text-sm text-destructive">
          {dialogError}
        </div>
      {/if}

      <div class="flex flex-col gap-2">
        <Label for="local-host-domain">Domain</Label>
        <Input
          id="local-host-domain"
          bind:value={domain}
          placeholder="router.lan"
          class="font-mono"
          disabled={isSaving}
          autocomplete="off"
        />
      </div>

      <div class="flex flex-col gap-2">
        <Label for="local-host-address">IPv4 or IPv6 Address</Label>
        <Input
          id="local-host-address"
          bind:value={address}
          placeholder="192.168.1.1 or fd00::1"
          class="font-mono"
          disabled={isSaving}
          autocomplete="off"
        />
      </div>

      <Dialog.Footer>
        <Button
          type="button"
          variant="outline"
          disabled={isSaving}
          onclick={() => (dialogOpen = false)}
        >
          Cancel
        </Button>
        <Button type="submit" disabled={isSaving}>
          {isSaving
            ? "Saving..."
            : dialogMode === "create"
              ? "Add Host"
              : "Save Changes"}
        </Button>
      </Dialog.Footer>
    </form>
  </Dialog.Content>
</Dialog.Root>
