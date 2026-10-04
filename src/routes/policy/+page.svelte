<script lang="ts">
  import { onMount } from "svelte";
  import { readText } from "@tauri-apps/plugin-clipboard-manager";
  import {
    coreProxyImportLink,
    coreProxyCreate,
    coreSetMode,
    coreUpdateProxy,
    coreDeleteProxy,
    coreCreatePolicy,
    coreUpdatePolicy,
    coreDeletePolicy,
    coreSelectPolicyMember,
  } from "$lib/core/api";
  import type {
    OutboundMode,
    Proxy,
    ProxyPolicy,
    ProxyPolicyType,
    ProxyServerConfig,
  } from "$lib/core/types";
  import { DEFAULT_POLICY_TAG } from "$lib/core/constants";

  import { profileStore } from "$lib/stores/profile.svelte";
  import { latencyStore } from "$lib/stores/latency.svelte";
  import { useAsyncAction } from "$lib/composables/use-async-action.svelte";
  import { toast } from "svelte-sonner";

  import PolicyHeader from "$lib/components/policy/policy-header.svelte";
  import OutboundModeTabs from "$lib/components/policy/outbound-mode.svelte";
  import PolicyGrid from "$lib/components/policy/policy-grid.svelte";
  import ProxyGrid from "$lib/components/policy/proxy-grid.svelte";
  import ProxySheet from "$lib/components/policy/proxy-sheet.svelte";
  import PolicySheet from "$lib/components/policy/policy-sheet.svelte";
  import ProxyChainSheet from "$lib/components/policy/proxy-chain-sheet.svelte";
  import { Skeleton } from "$lib/components/ui/skeleton/index.js";
  import * as AlertDialog from "$lib/components/ui/alert-dialog/index.js";
  import Spinner from "$lib/components/ui/spinner/spinner.svelte";

  // Dialogs
  type DialogKind =
    | "proxy"
    | "edit"
    | "policy"
    | "edit-policy"
    | "chain"
    | "edit-chain";
  let dialogOpen = $state(false);
  let dialogKind = $state<DialogKind>("proxy");
  let activeProxy = $state<Proxy | null>(null);
  let activePolicy = $state<ProxyPolicy | null>(null);

  // Proxy-delete confirmation. The backend does the real rule check and repoints
  // any dependent rules to Direct (Option A); the dialog message is generic.
  let deleteProxyOpen = $state(false);
  let proxyToDelete = $state<Proxy | null>(null);

  const action = useAsyncAction();

  // --- Derived ---
  // With the move to Profiles, the active profile is the single container: all
  // its proxies show at once (grouping was removed).
  const activeProfile = $derived(profileStore.activeProfile);
  const visibleProxies = $derived(profileStore.proxies);
  const canSelectProxy = $derived(profileStore.outboundMode === "global" || profileStore.outboundMode === "rule");

  // --- Lifecycle ---
  onMount(async () => {
    await action.run(async () => {
      await profileStore.load();
      await latencyStore.fetchLatenciesOnLoad();
    });
  });

  // --- Actions ---
  async function selectOutboundMode(mode: OutboundMode) {
    await action.run(async () => {
      await coreSetMode(mode);
      await profileStore.refresh();
    });
  }

  async function selectPolicyMemberAction(
    policyTag: string,
    memberTag: string,
  ) {
    await action.run(async () => {
      await coreSelectPolicyMember(policyTag, memberTag);
      await profileStore.refresh();
    });
  }

  function selectProxy(proxyTag: string) {
    selectPolicyMemberAction(DEFAULT_POLICY_TAG, proxyTag);
  }

  function selectPolicy(policyTag: string) {
    const p = profileStore.profile?.policies.find(
      (x) => x.tag === DEFAULT_POLICY_TAG,
    );
    const memberTag =
      policyTag === DEFAULT_POLICY_TAG
        ? (p?.selected_member_tag ?? DEFAULT_POLICY_TAG)
        : policyTag;
    selectPolicyMemberAction(DEFAULT_POLICY_TAG, memberTag);
  }

  function handleProxyClick(proxy: Proxy) {
    if (canSelectProxy && proxy.enabled) {
      selectProxy(proxy.tag);
    }
  }

  function handlePolicyClick(policy: ProxyPolicy) {
    if (canSelectProxy) {
      selectPolicy(policy.tag);
    }
  }

  async function importFromClipboard(providedText?: string) {
    await action.run(async () => {
      const text =
        typeof providedText === "string" ? providedText : await readText();
      const links = text
        .split("\n")
        .map((l) => l.trim())
        .filter((l) => l.includes("://"));
      if (links.length === 0)
        throw new Error("No proxy links found in clipboard.");

      for (const link of links) {
        try {
          await coreProxyImportLink(link);
        } catch {
          /* ignore individual failures */
        }
      }
      await profileStore.refresh();
      profileStore.pendingRestart = true;
    }, "Proxies imported from clipboard.");
  }

  function handlePaste(e: ClipboardEvent) {
    // Ignore if typing in an input
    if (
      e.target instanceof HTMLInputElement ||
      e.target instanceof HTMLTextAreaElement ||
      (e.target as HTMLElement).isContentEditable
    ) {
      return;
    }

    const text = e.clipboardData?.getData("text/plain");
    if (text) {
      e.preventDefault();
      importFromClipboard(text);
    }
  }

  // --- Delete actions ---
  // Always confirm before deleting a proxy — no pre-check on the frontend; the
  // backend checks rule usage and repoints dependents to Direct on confirm.
  function requestDeleteProxy(proxy: Proxy) {
    proxyToDelete = proxy;
    deleteProxyOpen = true;
  }

  async function confirmDeleteProxy() {
    if (!proxyToDelete) return;
    const tag = proxyToDelete.tag;
    await action.run(async () => {
      await coreDeleteProxy(tag);
      await profileStore.refresh();
      profileStore.pendingRestart = true;
      deleteProxyOpen = false;
      proxyToDelete = null;
    });
  }

  async function deletePolicy(policy: ProxyPolicy) {
    await action.run(async () => {
      await coreDeletePolicy(policy.tag);
      await profileStore.refresh();
      profileStore.pendingRestart = true;
    });
  }

  // --- Dialog saving ---
  async function saveProxy(config: ProxyServerConfig, title: string) {
    if (dialogKind === "proxy" || dialogKind === "chain") {
      await action.run(async () => {
        await coreProxyCreate({
          title: title || config.name || "New Proxy",
          config,
        });
        await profileStore.refresh();
        profileStore.pendingRestart = true;
        dialogOpen = false;
      });
    } else if (dialogKind === "edit" || dialogKind === "edit-chain") {
      if (!activeProxy) return;
      const proxyId = activeProxy.tag;
      await action.run(async () => {
        await coreUpdateProxy({
          tag: proxyId,
          title: title || undefined,
          config,
        });
        await profileStore.refresh();
        profileStore.pendingRestart = true;
        dialogOpen = false;
      });
    }
  }

  async function saveProxyChain(data: { name: string; proxies: string[] }) {
    const config: ProxyServerConfig = {
      tag: "", // backend allocates this
      name: data.name,
      address: "",
      port: 0,
      type: "chain",
      proxies: data.proxies,
    };
    await saveProxy(config, data.name);
  }

  async function savePolicy(
    name: string,
    type: ProxyPolicyType,
    members: string[],
    strategy?: string | null,
    tolerance_ms?: number | null,
    weights?: number[] | null,
  ) {
    if (!name.trim()) return;
    if (members.length === 0) {
      toast.error("Please select at least one member for the policy.");
      return;
    }
    if (dialogKind === "policy") {
      await action.run(async () => {
        await coreCreatePolicy({
          name: name.trim(),
          kind: type,
          members,
          selected_member_tag: type === "selector" ? members[0] : null,
          tolerance_ms:
            type === "balancer" || type === "urltest"
              ? (tolerance_ms ?? null)
              : null,
          strategy:
            type === "balancer" ? ((strategy as any) ?? "round-robin") : null,
          weights:
            type === "balancer" && strategy === "weighted"
              ? (weights ?? null)
              : null,
        });
        await profileStore.refresh();
        profileStore.pendingRestart = true;
        dialogOpen = false;
      });
    } else if (dialogKind === "edit-policy") {
      if (!activePolicy) return;
      await action.run(async () => {
        await coreUpdatePolicy({
          tag: activePolicy!.tag,
          name: name.trim(),
          kind: type,
          members,
          tolerance_ms: type === "balancer" ? (tolerance_ms ?? null) : null,
          strategy:
            type === "balancer" ? ((strategy as any) ?? "round-robin") : null,
          weights:
            type === "balancer" && strategy === "weighted"
              ? (weights ?? null)
              : null,
        });
        await profileStore.refresh();
        profileStore.pendingRestart = true;
        dialogOpen = false;
      });
    }
  }

  // --- Latency Tests ---
  async function runUrlTestAll() {
    await action.run(async () => {
      const err = await latencyStore.runUrlTestAll(visibleProxies);
      if (err) throw new Error(err);
    });
  }

  async function runUrlTestSingle(proxy: Proxy) {
    await action.run(async () => {
      const err = await latencyStore.runUrlTestSingle(proxy);
      if (err) throw new Error(err);
    });
  }

  function openProxyDialog(
    kind: "proxy" | "edit" | "chain" | "edit-chain",
    proxy: Proxy | null = null,
  ) {
    dialogKind = kind;
    activeProxy = proxy;
    dialogOpen = true;
  }

  function openPolicyDialog(
    kind: "policy" | "edit-policy",
    policy: ProxyPolicy | null = null,
  ) {
    dialogKind = kind;
    activePolicy = policy;
    dialogOpen = true;
  }
</script>

<svelte:window onpaste={handlePaste} />

<div class="flex flex-1 flex-col gap-6">
  <PolicyHeader
    testingAll={latencyStore.testingAll}
    isRunning={profileStore.isRunning}
    onTestAll={runUrlTestAll}
    onAddProxy={() => openProxyDialog("proxy")}
    onAddPolicy={() => openPolicyDialog("policy")}
    onAddProxyChain={() => openProxyDialog("chain")}
    onImportClipboard={importFromClipboard}
  >
    <OutboundModeTabs
      mode={profileStore.outboundMode}
      onSelectMode={selectOutboundMode}
    />
  </PolicyHeader>

  {#if profileStore.loading}
    <div class="flex flex-col gap-6 w-full animate-in fade-in duration-500">
      <div class="flex flex-col gap-3">
        <Skeleton class="h-6 w-32" />
        <div class="grid grid-cols-1 gap-3 md:grid-cols-2 lg:grid-cols-4">
          {#each Array(4) as _}
            <Skeleton class="h-24 w-full" />
          {/each}
        </div>
      </div>
      <div class="flex flex-col gap-3">
        <Skeleton class="h-6 w-32" />
        <div
          class="grid grid-cols-1 gap-2 sm:grid-cols-2 md:grid-cols-3 lg:grid-cols-4"
        >
          {#each Array(4) as _}
            <Skeleton class="h-24 w-full" />
          {/each}
        </div>
      </div>
    </div>
  {:else if profileStore.profile && activeProfile}
    <PolicyGrid
      policies={profileStore.visiblePolicies}
      {canSelectProxy}
      groupName={activeProfile.name}
      onPolicyClick={handlePolicyClick}
      onSelectPolicy={selectPolicy}
      onSelectMember={(policyTag, memberTag) =>
        selectPolicyMemberAction(policyTag, memberTag)}
      onEditPolicy={(policy) => openPolicyDialog("edit-policy", policy)}
      onDeletePolicy={deletePolicy}
    />

    <ProxyGrid
      proxies={visibleProxies}
      {canSelectProxy}
      groupName={activeProfile.name}
      onProxyClick={handleProxyClick}
      onEditProxy={(proxy) =>
        openProxyDialog(
          proxy.protocol === "chain" ? "edit-chain" : "edit",
          proxy,
        )}
      onSelectProxy={selectProxy}
      onTestProxy={runUrlTestSingle}
      onDeleteProxy={requestDeleteProxy}
      onAddProxy={() => openProxyDialog("proxy")}
    />
  {:else}
    <div
      class="text-muted-foreground flex flex-1 items-center justify-center py-12 text-sm"
    >
      No profile loaded.
    </div>
  {/if}
</div>

{#if dialogKind === "proxy" || dialogKind === "edit"}
  <ProxySheet
    bind:open={dialogOpen}
    mode={dialogKind === "proxy" ? "create" : "edit"}
    proxy={activeProxy}
    saving={action.saving}
    onSave={saveProxy}
  />
{:else if dialogKind === "chain" || dialogKind === "edit-chain"}
  <ProxyChainSheet
    bind:open={dialogOpen}
    mode={dialogKind === "chain" ? "create" : "edit"}
    proxy={activeProxy}
    allProxies={visibleProxies}
    saving={action.saving}
    onSave={saveProxyChain}
  />
{:else if dialogKind === "policy" || dialogKind === "edit-policy"}
  <PolicySheet
    bind:open={dialogOpen}
    mode={dialogKind === "policy" ? "create" : "edit"}
    policy={activePolicy}
    proxies={visibleProxies}
    saving={action.saving}
    onSave={savePolicy}
  />
{/if}

<AlertDialog.Root bind:open={deleteProxyOpen}>
  <AlertDialog.Content>
    <AlertDialog.Header>
      <AlertDialog.Title>Delete proxy?</AlertDialog.Title>
      <AlertDialog.Description>
        Are you sure you want to delete this proxy? It might be used in specific
        rules. Proceeding will fall back those rules to Direct.
      </AlertDialog.Description>
    </AlertDialog.Header>
    <AlertDialog.Footer>
      <AlertDialog.Cancel>Cancel</AlertDialog.Cancel>
      <AlertDialog.Action
        onclick={confirmDeleteProxy}
        disabled={action.saving}
        class="bg-destructive text-destructive-foreground hover:bg-destructive/90 w-18"
      >
        {#if action.saving}
          <Spinner class="border-white/30 border-t-white" />
        {:else}
          Delete
        {/if}
      </AlertDialog.Action>
    </AlertDialog.Footer>
  </AlertDialog.Content>
</AlertDialog.Root>
