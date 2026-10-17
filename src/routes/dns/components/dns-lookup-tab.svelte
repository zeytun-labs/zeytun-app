<script lang="ts">
  import { onDestroy } from "svelte";
  import { profileStore } from "$lib/stores/profile.svelte";
  import { Input } from "$lib/components/ui/input";
  import { Button } from "$lib/components/ui/button";
  import { Checkbox } from "$lib/components/ui/checkbox";
  import * as Select from "$lib/components/ui/select";
  import {
    testDnsLookup,
    testSystemDnsLookup,
    type DnsLookupResult,
    type DnsRecordType,
  } from "$lib/core/api";
  import { compareDnsAnswers, normalizeLookupDomain, type DnsComparison } from "$lib/core/dns-diagnostics";
  import { createDnsLoadingIndicator, shouldCompareSystem } from "$lib/core/dns-loading";
  import { LOCAL_DNS_SERVER_TAG } from "$lib/core/dns-servers";
  import { dnsResolverLabel, dnsLookupServerTag } from "$lib/core/dns-policy";
  import { errorMessage } from "$lib/errors";
  import DnsLookupResultPanel from "./dns-lookup-result.svelte";
  import LoadingIcon from "./loading-icon.svelte";

  const AUTOMATIC = "__automatic__";
  const recordTypes: DnsRecordType[] = ["A", "AAAA", "CNAME"];
  let domain = $state("");
  let recordType = $state<DnsRecordType>("A");
  let server = $state<string | null>(null);
  let bypassCache = $state(false);
  let compareSystem = $state(false);
  let coreLoading = $state(false);
  let systemLoading = $state(false);
  type DisplayedResponse = {
    domain: string;
    recordType: DnsRecordType;
    result: DnsLookupResult | null;
    error: string | null;
  };
  let coreResponse = $state<DisplayedResponse | null>(null);
  let systemResponse = $state<DisplayedResponse | null>(null);
  let formError = $state<string | null>(null);
  let pendingDomain = $state("");
  let pendingType = $state<DnsRecordType>("A");
  let requestId = $state(0);
  let comparisonResponse = $state<{
    domain: string; recordType: DnsRecordType; kind: DnsComparison;
  } | null>(null);
  let showLoading = $state(false);
  const loadingIndicator = createDnsLoadingIndicator((visible) => { showLoading = visible; });
  let generation = 0;
  onDestroy(() => {
    generation++;
    loadingIndicator.dispose();
  });

  const servers = $derived(profileStore.profile?.dns?.servers ?? []);
  const canCompare = $derived(recordType !== "CNAME");
  const willCompare = $derived(shouldCompareSystem(server, recordType, compareSystem));
  const isLoading = $derived(coreLoading || systemLoading);
  const busy = $derived(isLoading || showLoading);
  const serverLabel = $derived(server === null ? "Automatic · DNS rules" : dnsResolverLabel(server, servers));

  async function handleLookup(event: SubmitEvent) {
    event.preventDefault();
    if (busy) return;
    const query = normalizeLookupDomain(domain);
    if (!query) {
      formError = "Enter a domain name without a URL, path, or port.";
      return;
    }
    formError = null;
    const type = recordType;
    const selectedServer = server;
    const runtimeServer = dnsLookupServerTag(selectedServer, servers, profileStore.outboundMode);
    const skipCache = bypassCache;
    const compare = shouldCompareSystem(selectedServer, type, compareSystem);
    const current = ++generation;
    requestId++;
    pendingDomain = query;
    pendingType = type;
    coreLoading = true;
    systemLoading = compare;
    loadingIndicator.start();
    let nextCore: DisplayedResponse | null = null;
    let nextSystem: DisplayedResponse | null = null;

    async function runCore() {
      try {
        if (!profileStore.isRunning) throw new Error("Start the core to query its DNS resolver.");
        const result = await testDnsLookup(query!, type, runtimeServer, skipCache);
        nextCore = { domain: query!, recordType: type, result, error: null };
      } catch (error) {
        nextCore = { domain: query!, recordType: type, result: null, error: errorMessage(error) };
      } finally {
        if (generation === current) {
          coreResponse = nextCore;
          coreLoading = false;
        }
      }
    }
    async function runSystem() {
      try {
        const result = await testSystemDnsLookup(query!, type);
        nextSystem = { domain: query!, recordType: type, result, error: null };
      } catch (error) {
        nextSystem = { domain: query!, recordType: type, result: null, error: errorMessage(error) };
      } finally {
        if (generation === current) {
          systemResponse = nextSystem;
          systemLoading = false;
        }
      }
    }
    await Promise.allSettled([runCore(), ...(compare ? [runSystem()] : [])]);
    if (generation !== current) return;
    loadingIndicator.finish();
    if (!compare) systemResponse = null;
    // Compare only completed responses from the same query, never a stale pair.
    const core = nextCore as DisplayedResponse | null;
    const system = nextSystem as DisplayedResponse | null;
    comparisonResponse = core?.result && system?.result
      ? { domain: query, recordType: type, kind: compareDnsAnswers(core.result, system.result) }
      : null;
  }
</script>

<div class="flex w-full min-w-0 flex-col gap-4 px-2 pb-2">
  {#if !profileStore.isRunning}
    <p role="status" class="rounded-2xl border border-border/60 px-4 py-3 text-xs text-muted-foreground">
      Start Core to use Lookup. Enable Compare with System to query the system resolver.
    </p>
  {:else if profileStore.pendingRestart}
    <p role="status" class="rounded-2xl border border-border/60 bg-muted/20 px-4 py-3 text-xs text-muted-foreground">
      Results use active DNS settings. Restart Core to apply your saved changes.
    </p>
  {/if}

  <form onsubmit={handleLookup} class="flex min-w-0 flex-col gap-3">
    <div class="grid min-w-0 grid-cols-[6rem_minmax(0,1fr)] gap-2 lg:grid-cols-[minmax(0,1fr)_6rem_minmax(12rem,15rem)_7rem]">
      <Input
        aria-label="Domain to resolve"
        bind:value={domain}
        placeholder="Enter a domain, e.g. example.com"
        class="col-span-2 h-10 min-w-0 rounded-full lg:col-span-1 border-border/60 bg-card/40 px-4 font-mono text-sm"
        disabled={isLoading}
        spellcheck={false}
        autocapitalize="none"
        autocomplete="off"
      />
      <Select.Root type="single" value={recordType} disabled={isLoading} onValueChange={(value) => {
        if (recordTypes.includes(value as DnsRecordType)) recordType = value as DnsRecordType;
      }}>
        <Select.Trigger aria-label="DNS record type" class="data-[size=default]:h-10 w-full rounded-full font-mono">{recordType}</Select.Trigger>
        <Select.Content>
          {#each recordTypes as type (type)}<Select.Item value={type} label={type}>{type}</Select.Item>{/each}
        </Select.Content>
      </Select.Root>
      <Select.Root type="single" value={server ?? AUTOMATIC} disabled={isLoading} onValueChange={(value) => { server = value === AUTOMATIC ? null : value; }}>
        <Select.Trigger id="dns-lookup-resolver" aria-label="Resolver to query" class="data-[size=default]:h-10 w-full min-w-0 rounded-full"><span class="truncate">{serverLabel}</span></Select.Trigger>
        <Select.Content>
          <Select.Item value={AUTOMATIC} label="Automatic · DNS rules">Automatic · DNS rules</Select.Item>
          <Select.Item value={LOCAL_DNS_SERVER_TAG} label="Local (System)">Local (System)</Select.Item>
          {#each servers as item (item.tag)}<Select.Item value={item.tag} label={item.name}>{item.name}</Select.Item>{/each}
        </Select.Content>
      </Select.Root>
      <Button
        type="submit"
        class={`col-span-2 lg:col-span-1 h-10 w-full shrink-0 gap-2 rounded-full bg-primary px-4 hover:bg-primary disabled:bg-primary transition-[background-color,color,box-shadow,opacity] ${busy ? "disabled:opacity-100" : ""}`}
        disabled={busy || (!profileStore.isRunning && !willCompare)}
        aria-label="Lookup DNS records"
        aria-busy={isLoading}
      >
        <LoadingIcon loading={showLoading} />
        Lookup
      </Button>
    </div>

    <div class="flex flex-wrap items-center gap-x-6 gap-y-3 px-1 text-xs text-muted-foreground">
      <div class="flex items-center gap-2">
        <Checkbox id="dns-lookup-bypass" bind:checked={bypassCache} disabled={isLoading} />
        <label for="dns-lookup-bypass" class="cursor-pointer">Bypass Core cache</label>
      </div>
      {#if server === null}
        <div class="flex items-center gap-2">
          <Checkbox id="dns-lookup-system" checked={willCompare} onCheckedChange={(checked) => { compareSystem = checked; }} disabled={isLoading || !canCompare} />
          <label for="dns-lookup-system" class={canCompare ? "cursor-pointer" : "opacity-50"}>Compare with System</label>
        </div>
        {#if !canCompare}<span>System comparison supports A and AAAA, not CNAME.</span>{/if}
      {/if}
    </div>
    {#if server !== null || bypassCache}
      <p class="px-1 text-[11px] leading-relaxed text-muted-foreground">
        {server !== null ? "A specific resolver bypasses DNS rules, Hosts, and the FakeIP fallback. " : ""}
        {bypassCache ? "Bypass skips the Core DNS response cache for this query; FakeIP mappings and macOS cache are unchanged. " : ""}
        Advanced options require a compatible core.
      </p>
    {/if}
    {#if formError}<p role="alert" class="px-1 text-xs text-destructive">{formError}</p>{/if}
  </form>

  {#if comparisonResponse}
    <p role="status" class="rounded-2xl border border-border/50 bg-card/20 px-4 py-3 text-xs leading-relaxed text-muted-foreground">
      <span class="font-mono text-foreground">{comparisonResponse.domain} · {comparisonResponse.recordType}</span>
      <span aria-hidden="true" class="px-1 opacity-50">—</span>
      {#if comparisonResponse.kind === "same"}
        Core and System returned the same address set.
      {:else if comparisonResponse.kind === "fakeip"}
        One response contains FakeIP virtual addresses, so public IP equality is not expected. Different resolvers and CDN locations may also return different public addresses.
      {:else if comparisonResponse.kind === "different"}
        The address sets differ. Different DNS resolvers, CDN locations, and caches can return different valid answers. This comparison alone does not identify the cause.
      {:else}
        These responses cannot be compared as address sets because one has an error or no address answers.
      {/if}
    </p>
  {/if}
  {#if coreResponse || (coreLoading && showLoading)}
    <DnsLookupResultPanel
      title="Core"
      domain={coreResponse?.domain ?? pendingDomain}
      recordType={coreResponse?.recordType ?? pendingType}
      result={coreResponse?.result ?? null}
      error={coreResponse?.error ?? null}
      loading={coreLoading && showLoading}
      pendingDomain={pendingDomain}
      pendingType={pendingType}
      revision={requestId}
    />
  {/if}
  {#if systemResponse || (systemLoading && showLoading)}
    <DnsLookupResultPanel
      title="System"
      domain={systemResponse?.domain ?? pendingDomain}
      recordType={systemResponse?.recordType ?? pendingType}
      result={systemResponse?.result ?? null}
      error={systemResponse?.error ?? null}
      loading={systemLoading && showLoading}
      pendingDomain={pendingDomain}
      pendingType={pendingType}
      revision={requestId}
    />
  {/if}
</div>
