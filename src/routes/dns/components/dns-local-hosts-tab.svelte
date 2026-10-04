<script lang="ts">
  import { toast } from "svelte-sonner";
  import * as Card from "$lib/components/ui/card";
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

    let {
    triggerAdd = $bindable(),
    triggerDelete = $bindable(),
    selectedId = $bindable(null),
  } = $props<{
    triggerAdd?: () => void;
    triggerDelete?: () => void;
    selectedId?: string | null;
  }>();

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

    $effect(() => {
    triggerAdd = openCreateDialog;
    triggerDelete = async () => {
      if (selectedId) {
        await deleteHost(selectedId);
        selectedId = null;
      }
    };
  });
  
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
    const dns = profile.dns ?? {};
    dns.hosts = next;
    profile.dns = dns;
    profileStore.pendingRestart = true;
  }

  async function saveHost(event: SubmitEvent) {
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

  async function deleteHost(id: string) {
    if (isSaving) return;
    isSaving = true;
    operationError = null;
    try {
      await persistHosts(hosts.filter((host) => host.id !== id));
      toast.success("Local Hosts entry deleted.");
    } catch (error) {
      console.error("Failed to delete Local Hosts entry:", error);
      operationError = `Could not delete the Local Hosts entry: ${errorMessage(error)}`;
      toast.error(operationError);
    } finally {
      isSaving = false;
    }
  }

  async function toggleHost(id: string, enabled: boolean) {
    if (isSaving) return;
    isSaving = true;
    operationError = null;
    try {
      await persistHosts(
        hosts.map((host) => (host.id === id ? { ...host, enabled } : host)),
      );
    } catch (error) {
      console.error("Failed to toggle Local Hosts entry:", error);
      operationError = `Could not update the Local Hosts entry: ${errorMessage(error)}`;
      toast.error(operationError);
    } finally {
      isSaving = false;
    }
  }
</script>

<div class="flex flex-col w-full text-sm">
  {#if operationError}
    <div class="mb-3 rounded-xl border border-destructive/30 bg-destructive/10 p-3 text-sm text-destructive">
      {operationError}
    </div>
  {/if}

  {#if hosts.length === 0}
    <div class="flex flex-col items-center justify-center gap-2 rounded-2xl border border-dashed border-border/40 py-12 text-center bg-card/40 mt-1">
      <HugeiconsIcon icon={ComputerIcon} class="size-8 text-muted-foreground/40" />
      <p class="text-sm font-medium text-muted-foreground">No local host overrides</p>
      <p class="max-w-sm text-xs text-muted-foreground/70 mb-2">
        Add a domain and IPv4 or IPv6 address to create a static DNS mapping.
      </p>
    </div>
  {:else}
    <div
      class="sticky top-0 uppercase text-xs bg-background z-20 grid grid-cols-[60px_1fr_1fr] px-2 border-b border-border/70"
    >
      <div class="h-8 flex items-center font-medium text-muted-foreground px-2"></div>
      <div class="h-8 flex items-center font-medium text-muted-foreground px-2">Domain</div>
      <div class="h-8 flex items-center font-medium text-muted-foreground px-2">Address</div>
    </div>
    <div class="flex flex-col">
      {#each hosts as host, i (host.id)}
        {#if i > 0}<div class="h-px w-full bg-border/40"></div>{/if}
        <!-- svelte-ignore a11y_click_events_have_key_events -->
        <!-- svelte-ignore a11y_no_static_element_interactions -->
        <div
          data-state={selectedId === host.id ? "selected" : undefined}
          class="h-12 grid grid-cols-[60px_1fr_1fr] items-center rounded-xl hover:bg-muted/30 data-[state=selected]:bg-muted/40 transition-colors px-2 cursor-pointer {host.enabled ? '' : 'opacity-50'}"
          onclick={() => { selectedId = host.id; }}
          ondblclick={() => openEditDialog(host)}
        >
          <div class="px-2 flex items-center justify-center">
            <CellEnabled
              checked={host.enabled}
              disabled={isSaving}
              onChange={(enabled) => toggleHost(host.id, enabled)}
            />
          </div>
          <div class="px-2 truncate font-mono font-medium text-foreground" title={host.domain}>
            {host.domain}
          </div>
          <div class="px-2 truncate font-mono text-muted-foreground" title={host.address}>
            {host.address}
          </div>
        </div>
      {/each}
    </div>
  {/if}
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
