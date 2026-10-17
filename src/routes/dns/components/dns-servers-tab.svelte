<script lang="ts">
  import { onMount } from "svelte";
  import { toast } from "svelte-sonner";
  import * as Dialog from "$lib/components/ui/dialog/index.js";
  import * as Empty from "$lib/components/ui/empty/index.js";
  import { Input } from "$lib/components/ui/input/index.js";
  import { Button } from "$lib/components/ui/button/index.js";
  import { Label } from "$lib/components/ui/label/index.js";
  import { Skeleton } from "$lib/components/ui/skeleton/index.js";
  import * as Select from "$lib/components/ui/select/index.js";
  import { HugeiconsIcon } from "@hugeicons/svelte";
  import {
    Add01Icon,
    ServerStack01Icon,
  } from "@hugeicons/core-free-icons";
  import { profileStore } from "$lib/stores/profile.svelte";
  import type { DnsServer } from "$lib/core/types";
  import { FOLLOW_MODE_DETOUR } from "$lib/core/dns-policy";
  import {
    DNS_PROTOCOLS,
    dnsProtocolFromAddress,
    dnsAddressWithoutProtocol,
    upsertDnsServer,
    dnsServerDeleteError,
    dnsDetourExists,
    type DnsProtocol,
  } from "$lib/core/dns-servers";
  import DnsRowActions from "$lib/components/rule/dns-row-actions.svelte";
  import Badge from "$lib/components/ui/badge/badge.svelte";
  import { cn } from "$lib/utils";
  import { errorMessage } from "$lib/errors";

  type UIItem = {
    tag: string;
    address: string;
    detour: string | null;
    protocol: DnsProtocol;
    name: string;
  };

  let {
    triggerAdd = $bindable(),
  } = $props<{
    triggerAdd?: () => void;
  }>();

  let selectedId = $state<string | null>(null);
  let isSaving = $state(false);
  let operationError = $state<string | null>(null);

  // The profile is the only source of truth; never display unpersisted servers.
  const rawServers = $derived<DnsServer[]>(
    profileStore.profile?.dns?.servers ?? [],
  );

  // All servers combined
  const allServers = $derived<UIItem[]>(
    rawServers.map((s) => ({
      tag: s.tag,
      address: s.address,
      detour: s.detour ?? null,
      protocol: dnsProtocolFromAddress(s.address),
      name: s.name,
    })),
  );

  // Creating and editing use the same form; editing never replaces a server tag.
  let dialogOpen = $state(false);
  let editingTag = $state<string | null>(null);
  let dialogProfileId = $state<string | null>(null);
  let dialogError = $state<string | null>(null);
  let newAddress = $state("");
  let newName = $state("");
  let newProtocol = $state<DnsProtocol>("DoH");
  // New servers follow connection mode; legacy missing detours remain Direct.
  let newDetour = $state<string | null>(FOLLOW_MODE_DETOUR);
  const proxyOptions = $derived(
    (profileStore.profile?.proxies ?? []).map((proxy) => ({
      tag: proxy.tag,
      label: `${proxy.title || proxy.tag}${!proxy.enabled ? " (disabled)" : !proxy.available ? " (unavailable)" : ""}`,
    })),
  );

  onMount(async () => {
    if (!profileStore.profile) {
      try {
        await profileStore.load();
      } catch (error) {
        operationError = errorMessage(error);
      }
    }
  });

  function openAddDialog() {
    if (isSaving || profileStore.loading || !profileStore.profile) return;
    editingTag = null;
    dialogProfileId = profileStore.profile.id;
    dialogError = null;
    operationError = null;
    newAddress = "";
    newName = "";
    newProtocol = "DoH";
    newDetour = FOLLOW_MODE_DETOUR;
    dialogOpen = true;
  }

  function openEditDialog(tag: string) {
    if (isSaving || profileStore.loading) return;
    const server = rawServers.find((item) => item.tag === tag);
    if (!server || !profileStore.profile) return;
    selectedId = tag;
    editingTag = tag;
    dialogProfileId = profileStore.profile.id;
    dialogError = null;
    operationError = null;
    newName = server.name;
    newAddress = dnsAddressWithoutProtocol(server.address);
    newProtocol = dnsProtocolFromAddress(server.address);
    newDetour = server.detour || "direct";
    dialogOpen = true;
  }

  $effect(() => {
    triggerAdd = openAddDialog;
  });

  function changeProtocol(protocol: DnsProtocol) {
    newProtocol = protocol;
    newAddress = dnsAddressWithoutProtocol(newAddress);
    dialogError = null;
  }

  function handleAddressInput(event: Event) {
    const input = event.currentTarget as HTMLInputElement;
    if (/^(https|tls|tcp|udp|quic):\/\//i.test(input.value.trim())) {
      newProtocol = dnsProtocolFromAddress(input.value);
    }
  }

  async function persistServers(servers: DnsServer[]) {
    const profile = profileStore.profile;
    if (!profile) throw new Error("No active profile is available.");
    await profileStore.updateProfile({
      id: profile.id,
      dns: { ...profile.dns, servers },
    });
  }

  async function saveServer(event: SubmitEvent) {
    event.preventDefault();
    if (isSaving) return;
    dialogError = null;
    isSaving = true;
    try {
      const profile = profileStore.profile;
      if (!profile || profile.id !== dialogProfileId) {
        throw new Error("The active profile changed. Close this dialog and try again.");
      }
      const selectedDetour = newDetour?.trim() || null;
      const detour = selectedDetour?.toLowerCase() === "direct" ? "direct" : selectedDetour;
      if (!dnsDetourExists(detour, profile)) {
        throw new Error("The selected proxy is no longer available. Select another outbound.");
      }
      const tag = editingTag ?? `dns-${crypto.randomUUID()}`;
      const nextServers = upsertDnsServer(
        rawServers,
        { name: newName, address: newAddress, protocol: newProtocol, detour },
        tag,
        editingTag !== null,
      );
      await persistServers(nextServers);
      selectedId = tag;
      toast.success(editingTag ? "DNS server updated." : "DNS server added.");
      dialogOpen = false;
    } catch (error) {
      dialogError = errorMessage(error);
    } finally {
      isSaving = false;
    }
  }

  async function removeServer(tag: string) {
    if (isSaving || profileStore.loading) return;
    operationError = null;
    const dns = profileStore.profile?.dns;
    if (!dns || !rawServers.some((server) => server.tag === tag)) return;
    const referenceError = dnsServerDeleteError(dns, tag);
    if (referenceError) {
      operationError = referenceError;
      return;
    }
    isSaving = true;
    try {
      await persistServers(rawServers.filter((server) => server.tag !== tag));
      if (selectedId === tag) selectedId = null;
      toast.success("DNS server removed.");
    } catch (error) {
      operationError = errorMessage(error);
    } finally {
      isSaving = false;
    }
  }

  function protocolColor(protocol: DnsProtocol) {
    switch (protocol) {
      case "DoH":
        return "border-sky-500/20 bg-sky-500/10 text-sky-500";
      case "DoT":
        return "border-violet-500/20 bg-violet-500/10 text-violet-500";
      case "QUIC":
        return "border-amber-500/20 bg-amber-500/10 text-amber-500";
      case "TCP":
        return "border-orange-500/20 bg-orange-500/10 text-orange-500";
      case "UDP":
        return "border-teal-500/20 bg-teal-500/10 text-teal-500";
    }
  }

  function getPolicyName(name: string | null | undefined, tag: string): string {
    const raw = name || tag;
    return raw === "ROOT_POLICY" || raw === "root-policy"
      ? "Global Proxy"
      : raw;
  }

  function detourLabel(detour: string | null): string {
    if (detour === FOLLOW_MODE_DETOUR) return "Follow connection mode";
    if (!detour) return "Direct";
    if (detour === "direct" || detour === "DIRECT") return "Direct";
    const policy = profileStore.profile?.policies?.find(
      (p) => p.tag === detour,
    );
    if (policy) return getPolicyName(policy.name, policy.tag);
    return proxyOptions.find((proxy) => proxy.tag === detour)?.label ?? `${detour} (missing outbound)`;
  }
</script>

{#snippet serverRow(server: UIItem)}
  <DnsRowActions
    label={`DNS server ${server.name}`}
    selected={selectedId === server.tag}
    disabled={isSaving || profileStore.loading}
    class="group h-12 grid grid-cols-[80px_160px_1fr_160px_40px] sm:grid-cols-[80px_200px_1fr_180px_40px] items-center rounded-xl hover:bg-card/70 data-[state=selected]:bg-card/70 transition-colors cursor-pointer px-2"
    onSelect={() => { selectedId = server.tag; }}
    onEdit={() => openEditDialog(server.tag)}
    onDelete={() => { void removeServer(server.tag); }}
  >
    <!-- Protocol badge -->
    <div class="px-2 flex items-center">
      <Badge class={cn("font-mono", protocolColor(server.protocol))}>
        {server.protocol}
      </Badge>
    </div>

    <!-- Name -->
    <div
      class="px-2 truncate font-medium text-foreground"
      title={server.name || "—"}
    >
      {server.name || "—"}
    </div>

    <!-- Address -->
    <div
      class="px-2 truncate font-mono  text-muted-foreground"
      title={server.address}
    >
      {server.address}
    </div>

    <!-- Detour badge -->
    <div class="px-2 flex items-center">
      <Badge variant="secondary" class="text-muted-foreground">
        {detourLabel(server.detour)}
      </Badge>
    </div>
  </DnsRowActions>
{/snippet}

<div class="w-full text-sm px-2 pb-2 flex flex-col">
  {#if operationError}
    <p role="alert" class="m-2 rounded-xl border border-destructive/30 bg-destructive/10 p-3 text-xs text-destructive">{operationError}</p>
  {/if}
  {#if profileStore.loading}
    <div class="flex flex-col gap-3 p-4" aria-label="Loading DNS servers">
      {#each [1, 2, 3] as row (row)}
        <Skeleton class="h-11 w-full" />
      {/each}
    </div>
  {:else if allServers.length === 0}
    <div class="mt-4">
      <Empty.Root class="border border-dashed border-border/50 bg-card/20 py-12">
        <Empty.Media variant="icon">
          <HugeiconsIcon
            icon={ServerStack01Icon}
            class="size-5 text-muted-foreground"
          />
        </Empty.Media>
        <Empty.Header>
          <Empty.Title>No DNS servers configured</Empty.Title>
          <Empty.Description>
            Add upstream DNS servers (DoH, DoT, UDP, TCP) to resolve domain queries.
          </Empty.Description>
        </Empty.Header>
        <Empty.Content>
          <Button
            variant="outline"
            size="sm"
            class="gap-1.5"
            onclick={openAddDialog}
            disabled={isSaving || profileStore.loading}
          >
            <HugeiconsIcon icon={Add01Icon} class="size-4" />
            Add Server
          </Button>
        </Empty.Content>
      </Empty.Root>
    </div>
  {:else}
    <div
      class="sticky top-0 uppercase text-xs bg-background z-20 grid grid-cols-[80px_160px_1fr_160px_40px] sm:grid-cols-[80px_200px_1fr_180px_40px] px-2 border-b border-border/70"
    >
      <div class="h-8 flex items-center font-medium text-muted-foreground px-2">
        Type
      </div>
      <div class="h-8 flex items-center font-medium text-muted-foreground px-2">
        Name
      </div>
      <div class="h-8 flex items-center font-medium text-muted-foreground px-2">
        Address
      </div>
      <div class="h-8 flex items-center font-medium text-muted-foreground px-2">
        Routing
      </div>
      <div class="sr-only">Actions</div>
    </div>
    <div class="space-y-0 text-sm">
      {#each allServers as server, i (server.tag)}
        {@render serverRow(server)}
        {#if i < allServers.length - 1}
          <div class="h-px w-full bg-border/40"></div>
        {/if}
      {/each}
    </div>
  {/if}
</div>

<!-- The existing server tag is retained when editing. -->
<Dialog.Root bind:open={dialogOpen}>
  <Dialog.Content class="sm:max-w-md flex flex-col">
    <form onsubmit={saveServer} class="flex flex-col gap-4">
      <Dialog.Header>
        <Dialog.Title>{editingTag ? "Edit DNS Server" : "Add DNS Server"}</Dialog.Title>
        <Dialog.Description>
          {editingTag ? "Update this resolver. DNS rules that use it keep their target." : "Choose a resolver and the outbound used for its DNS requests."}
        </Dialog.Description>
      </Dialog.Header>
      {#if dialogError}
        <p role="alert" class="rounded-xl border border-destructive/30 bg-destructive/10 p-3 text-xs text-destructive">{dialogError}</p>
      {/if}
      <div class="flex flex-col gap-4">
        <div class="flex flex-col gap-2">
          <Label for="dns-name">Server Name</Label>
          <Input
            id="dns-name"
            bind:value={newName}
            disabled={isSaving}
            required
            placeholder="e.g. Google DNS"
          />
        </div>
        <div class="flex flex-col gap-2">
          <Label for="dns-address">Server Address</Label>
          <Input
            id="dns-address"
            bind:value={newAddress}
            oninput={handleAddressInput}
            disabled={isSaving}
            required
            aria-describedby="dns-address-hint"
            placeholder={newProtocol === "DoH"
              ? "dns.google/dns-query"
              : newProtocol === "DoT"
                ? "dns.google:853"
                : "223.5.5.5"}
          />
          <p id="dns-address-hint" class="text-xs text-muted-foreground">
            {#if newProtocol === "DoH"}
              HTTPS encrypts queries. The default path is /dns-query; keep a custom
              path if your provider requires one.
            {:else}
              {newProtocol === "DoT" || newProtocol === "QUIC" ? "Encrypted" : "Unencrypted"} DNS.
              Enter a hostname or IP address and optional port. Use [IPv6]:port for IPv6.
            {/if}
          </p>
        </div>
        <div class="flex flex-col gap-2">
          <Label>Protocol</Label>
          <Select.Root
            type="single"
            value={newProtocol}
            disabled={isSaving}
            onValueChange={(v) => {
              if (DNS_PROTOCOLS.includes(v as DnsProtocol)) changeProtocol(v as DnsProtocol);
            }}
          >
            <Select.Trigger class="w-full justify-between">
              {newProtocol}
            </Select.Trigger>
            <Select.Content>
              {#each DNS_PROTOCOLS as proto (proto)}
                <Select.Item value={proto} label={proto}>
                  {proto}
                </Select.Item>
              {/each}
            </Select.Content>
          </Select.Root>
        </div>
        <div class="flex flex-col gap-2">
          <Label>Outbound</Label>
          <Select.Root
            type="single"
            value={newDetour ?? "direct"}
            disabled={isSaving}
            onValueChange={(v) => {
              newDetour = v;
            }}
          >
            <Select.Trigger class="w-full justify-between">
              {detourLabel(newDetour)}
            </Select.Trigger>
            <Select.Content>
              <Select.Item value={FOLLOW_MODE_DETOUR} label="Follow connection mode">
                Follow connection mode
              </Select.Item>
              <Select.Item value="direct" label="Direct">
                Direct
              </Select.Item>
              <Select.Group>
                <Select.Label>Proxy Policies</Select.Label>
                {#each profileStore.profile?.policies ?? [] as policy (policy.tag)}
                  <Select.Item
                    value={policy.tag}
                    label={getPolicyName(policy.name, policy.tag)}
                  >
                    {getPolicyName(policy.name, policy.tag)}
                  </Select.Item>
                {/each}
              </Select.Group>
              {#if proxyOptions.length > 0}
                <Select.Group>
                  <Select.Label>Individual Proxies</Select.Label>
                  {#each proxyOptions as proxy (proxy.tag)}
                    <Select.Item value={proxy.tag} label={proxy.label}>
                      {proxy.label}
                    </Select.Item>
                  {/each}
                </Select.Group>
              {/if}
            </Select.Content>
          </Select.Root>
          <p class="text-[11px] text-muted-foreground">
            Follow connection mode uses your selected proxy in Global and Rule modes,
            and connects directly in Direct mode. An explicit outbound keeps its chosen route.
          </p>
        </div>
      </div>
      <Dialog.Footer>
        <Button type="button" variant="outline" onclick={() => (dialogOpen = false)} disabled={isSaving}>
          Cancel
        </Button>
        <Button type="submit" disabled={isSaving || profileStore.loading}>{isSaving ? "Saving…" : editingTag ? "Save Changes" : "Add Server"}</Button>
      </Dialog.Footer>
    </form>
  </Dialog.Content>
</Dialog.Root>
