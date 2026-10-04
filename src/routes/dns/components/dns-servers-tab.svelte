<script lang="ts">
  import { onMount } from "svelte";
  import { toast } from "svelte-sonner";
  import * as Dialog from "$lib/components/ui/dialog/index.js";
  import { Input } from "$lib/components/ui/input/index.js";
  import { Button } from "$lib/components/ui/button/index.js";
  import { Label } from "$lib/components/ui/label/index.js";
  import * as Select from "$lib/components/ui/select/index.js";
  import { HugeiconsIcon } from "@hugeicons/svelte";
  import {
    Add01Icon,
    Delete01Icon,
    ServerStack01Icon,
  } from "@hugeicons/core-free-icons";
  import { profileStore } from "$lib/stores/profile.svelte";
  import type { DnsServer } from "$lib/core/types";
  import Badge from "$lib/components/ui/badge/badge.svelte";
  import { cn } from "$lib/utils";
  import { errorMessage } from "$lib/errors";

  type DnsProtocol = "DoH" | "DoT" | "TCP" | "UDP" | "QUIC";

  type UIItem = {
    tag: string;
    address: string;
    detour: string | null;
    protocol: DnsProtocol;
    name: string;
  };

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

  // The profile is the only source of truth; never display unpersisted servers.
  const rawServers = $derived<DnsServer[]>(
    profileStore.profile?.dns?.servers ?? [],
  );

  function getProtocolFromAddress(address: string): DnsProtocol {
    if (address.startsWith("https://")) return "DoH";
    if (address.startsWith("tls://")) return "DoT";
    if (address.startsWith("quic://")) return "QUIC";
    if (address.startsWith("tcp://")) return "TCP";
    return "UDP";
  }

  // All servers combined
  const allServers = $derived<UIItem[]>(
    rawServers.map((s) => ({
      tag: s.tag,
      address: s.address,
      detour: s.detour ?? null,
      protocol: getProtocolFromAddress(s.address),
      name: s.name,
    })),
  );

  // --- Add Dialog State ---
  let addDialogOpen = $state(false);
  let newAddress = $state("");
  let newName = $state("");
  let newProtocol = $state<DnsProtocol>("DoH");
  // Default to null (no detour/core default). User can change to "direct" or a policy.
  let newDetour = $state<string | null>(null);

  const protocols: DnsProtocol[] = ["DoH", "DoT", "TCP", "UDP", "QUIC"];

  onMount(async () => {
    if (!profileStore.profile) {
      await profileStore.load();
    }
  });

  function openAddDialog() {
    newAddress = "";
    newName = "";
    newProtocol = "DoH";
    newDetour = null;
    addDialogOpen = true;
  }

  $effect(() => {
    triggerAdd = openAddDialog;
    triggerDelete = async () => {
      if (selectedId) {
        await removeServer(selectedId);
        selectedId = null;
      }
    };
  });

  function formatAddress(raw: string, protocol: DnsProtocol): string {
    let clean = raw.trim();
    if (!clean) return "";
    // Strip any existing protocol prefix first
    clean = clean.replace(
      /^(https?:\/\/|tls:\/\/|quic:\/\/|tcp:\/\/|udp:\/\/)/i,
      "",
    );
    switch (protocol) {
      case "DoH":
        return `https://${clean}`;
      case "DoT":
        return `tls://${clean}`;
      case "QUIC":
        return `quic://${clean}`;
      case "TCP":
        return `tcp://${clean}`;
      case "UDP":
        return clean;
    }
  }

  async function persistServers(servers: DnsServer[]) {
    if (!profileStore.profile) {
      throw new Error("No active profile is available.");
    }
    isSaving = true;
    const currentDns = profileStore.profile.dns ?? {};
    try {
      await profileStore.updateProfile({
        id: profileStore.profile.id,
        dns: {
          ...currentDns,
          servers,
        },
      });
      profileStore.pendingRestart = true;
    } finally {
      isSaving = false;
    }
  }

  async function addServer() {
    const raw = newAddress.trim();
    if (!raw) {
      toast.error("Server address is required.");
      return;
    }
    const rawName = newName.trim();
    if (!rawName) {
      toast.error("Server name is required.");
      return;
    }

    const formattedAddress = formatAddress(raw, newProtocol);
    const tag = `dns-${Date.now().toString(36)}`;

    const newServer: DnsServer = {
      tag,
      address: formattedAddress,
      detour: newDetour ?? undefined,
      name: rawName,
    };

    const nextServers = [...rawServers, newServer];
    try {
      await persistServers(nextServers);
      toast.success("DNS server added successfully.");
      addDialogOpen = false;
    } catch (error) {
      toast.error("Failed to add DNS server", {
        description: errorMessage(error),
      });
      console.error("Failed to add DNS server:", error);
    }
  }

  async function removeServer(tag: string) {
    const nextServers = rawServers.filter((s) => s.tag !== tag);
    try {
      await persistServers(nextServers);
      toast.success("DNS server removed.");
    } catch (error) {
      toast.error("Failed to remove DNS server", {
        description: errorMessage(error),
      });
      console.error("Failed to remove DNS server:", error);
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
    if (!detour) return "default";
    if (detour === "direct" || detour === "DIRECT") return "direct (Local DNS)";
    const policy = profileStore.profile?.policies?.find(
      (p) => p.tag === detour,
    );
    return policy ? getPolicyName(policy.name, policy.tag) : detour;
  }
</script>

{#snippet serverRow(server: UIItem)}
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    data-state={selectedId === server.tag ? "selected" : undefined}
    class="group h-12 grid grid-cols-[80px_160px_1fr_160px] sm:grid-cols-[80px_200px_1fr_180px] items-center rounded-xl hover:bg-card/70 data-[state=selected]:bg-card/70 transition-colors cursor-pointer px-2"
    onclick={() => {
      selectedId = server.tag;
    }}
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
  </div>
{/snippet}

<div class="w-full text-sm px-2 pb-2 flex flex-col">
  {#if allServers.length === 0}
    <div
      class="flex flex-col items-center justify-center gap-2 rounded-2xl border border-dashed border-border/40 py-12 text-center bg-card/40 mt-4"
    >
      <HugeiconsIcon
        icon={ServerStack01Icon}
        class="size-8 text-muted-foreground/40"
      />
      <p class="text-sm text-muted-foreground">No servers configured.</p>
      <Button
        variant="outline"
        size="sm"
        class="mt-1"
        onclick={openAddDialog}
        disabled={isSaving || profileStore.loading}
      >
        <HugeiconsIcon
          icon={Add01Icon}
          class="size-4"
          data-icon="inline-start"
        />
        Add your first server
      </Button>
    </div>
  {:else}
    <div
      class="sticky top-0 uppercase text-xs bg-background z-20 grid grid-cols-[80px_160px_1fr_160px] sm:grid-cols-[80px_200px_1fr_180px] px-2 border-b border-border/70"
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

<!-- Add Server Dialog -->
<Dialog.Root bind:open={addDialogOpen}>
  <Dialog.Content class="sm:max-w-md flex flex-col">
    <Dialog.Header>
      <Dialog.Title>Add DNS Server</Dialog.Title>
      <Dialog.Description>Configure a new DNS server.</Dialog.Description>
    </Dialog.Header>
    <div class="flex flex-col gap-4">
      <div class="flex flex-col gap-2">
        <Label for="dns-name">Server Name</Label>
        <Input
          id="dns-name"
          bind:value={newName}
          placeholder="e.g. Google DNS"
        />
      </div>
      <div class="flex flex-col gap-2">
        <Label for="dns-address">Server Address</Label>
        <Input
          id="dns-address"
          bind:value={newAddress}
          placeholder={newProtocol === "DoH"
            ? "dns.google/dns-query"
            : newProtocol === "DoT"
              ? "1.1.1.1"
              : "223.5.5.5"}
        />
      </div>
      <div class="flex flex-col gap-2">
        <Label>Protocol</Label>
        <Select.Root
          type="single"
          value={newProtocol}
          onValueChange={(v) => {
            newProtocol = v as DnsProtocol;
          }}
        >
          <Select.Trigger class="w-full justify-between">
            {newProtocol}
          </Select.Trigger>
          <Select.Content>
            {#each protocols as proto (proto)}
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
          value={newDetour ?? ""}
          onValueChange={(v) => {
            newDetour = v === "" ? null : v;
          }}
        >
          <Select.Trigger class="w-full justify-between">
            {newDetour === "direct" || newDetour === "DIRECT"
              ? "direct (Local DNS)"
              : newDetour
                ? detourLabel(newDetour)
                : "No detour (core default)"}
          </Select.Trigger>
          <Select.Content>
            <Select.Item value="" label="No detour (core default)">
              No detour (core default)
            </Select.Item>
            <Select.Item value="direct" label="direct (Local DNS)">
              direct (Local DNS)
            </Select.Item>
            {#each profileStore.profile?.policies ?? [] as policy (policy.tag)}
              <Select.Item
                value={policy.tag}
                label={getPolicyName(policy.name, policy.tag)}
              >
                {getPolicyName(policy.name, policy.tag)}
              </Select.Item>
            {/each}
          </Select.Content>
        </Select.Root>
        <p class="text-[11px] text-muted-foreground">
          Choose which outbound this DNS server queries through. Select "direct
          (Local DNS)" for domain-based local routing.
        </p>
      </div>
    </div>
    <Dialog.Footer>
      <Button variant="outline" onclick={() => (addDialogOpen = false)}
        >Cancel</Button
      >
      <Button onclick={addServer} disabled={isSaving}>Add Server</Button>
    </Dialog.Footer>
  </Dialog.Content>
</Dialog.Root>
