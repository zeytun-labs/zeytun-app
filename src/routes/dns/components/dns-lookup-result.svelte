<script lang="ts">
  import { onDestroy } from "svelte";
  import { Button } from "$lib/components/ui/button";
  import { Spinner } from "$lib/components/ui/spinner";
  import { HugeiconsIcon } from "@hugeicons/svelte";
  import { Copy01Icon, Tick01Icon } from "@hugeicons/core-free-icons";
  import { toast } from "svelte-sonner";
  import type { DnsLookupResult, DnsRecordType } from "$lib/core/api";
  import { isFakeIp, lookupRecords } from "$lib/core/dns-diagnostics";
  import { dnsResolverLabel } from "$lib/core/dns-policy";
  import { profileStore } from "$lib/stores/profile.svelte";

  let { title, domain, recordType, result, error = null, loading = false, pendingDomain = "", pendingType = "A", revision = 0 }: {
    title: "Core" | "System";
    domain: string;
    recordType: DnsRecordType;
    result: DnsLookupResult | null;
    error?: string | null;
    loading?: boolean;
    pendingDomain?: string;
    pendingType?: DnsRecordType;
    revision?: number;
  } = $props();

  let copiedRevision = $state<number | null>(null);
  let copiedResult = $state<DnsLookupResult | null>(null);
  const copied = $derived(copiedRevision === revision && copiedResult === result && result !== null);
  const updating = $derived(loading && (result !== null || error !== null));
  let copyTimeout: ReturnType<typeof setTimeout> | undefined;
  onDestroy(() => clearTimeout(copyTimeout));

  const records = $derived(result ? lookupRecords(result) : []);
  const sections = ["answer", "authority", "additional"] as const;
  const groups = $derived(sections.map((section) => ({
    section, records: records.filter((record) => record.section === section),
  })).filter((group) => group.records.length > 0));
  const hasAnswers = $derived(records.some((record) => record.section === "answer"));
  const hasFakeIp = $derived(result?.resolved_ips.some(isFakeIp) ?? false);
  const trace = $derived(result?.trace ?? null);
  const resolverId = $derived(trace?.server || result?.server_used || null);
  const resolverLabel = $derived(title === "System" ? "macOS system resolver" :
    dnsResolverLabel(resolverId, profileStore.profile?.dns?.servers ?? []));
  const traceFields = $derived([
    { label: "Resolver ID", value: resolverId },
    ...(trace ? [
    { label: "Selection", value: trace.selection },
    { label: "Transport", value: trace.transport },
    { label: "Matched rule", value: trace.rule },
    { label: "Rule position", value: trace.rule_index === null ? null : `${trace.rule_index + 1}` },
    { label: "Configured outbound", value: trace.outbound },
    { label: "Source", value: trace.source },
    { label: "Cache", value: trace.cache_hit === null ? null : trace.cache_hit ? "Hit" : "Miss" },
    ] : []),
  ].filter((entry) => entry.value !== null && entry.value !== undefined && entry.value !== ""));

  const statusNames: Record<number, string> = {
    0: "SUCCESS", 1: "FORMERR", 2: "SERVFAIL", 3: "NXDOMAIN", 4: "NOTIMP",
    5: "REFUSED", 6: "YXDOMAIN", 7: "YXRRSET", 8: "NXRRSET", 9: "NOTAUTH", 10: "NOTZONE",
  };
  const status = $derived(result?.status === null ? "COMPLETED" : statusNames[result?.status ?? -1] ?? "DNS ERROR");

  async function copyRecords() {
    const response = result;
    const responseRevision = revision;
    try {
      await navigator.clipboard.writeText(records.map((record) =>
        [record.section, record.name || domain, record.record_type, record.data, record.ttl ?? ""].join("\t"),
      ).join("\n"));
      copiedResult = response;
      copiedRevision = responseRevision;
      clearTimeout(copyTimeout);
      copyTimeout = setTimeout(() => { copiedRevision = null; copiedResult = null; }, 2000);
      toast.success("DNS records copied.");
    } catch {
      toast.error("Could not copy DNS records.");
    }
  }
</script>

<section aria-label={`${title} DNS result`} aria-live="polite" class="min-w-0 overflow-hidden rounded-2xl border border-border/60 bg-card/30 motion-safe:animate-in motion-safe:fade-in motion-safe:slide-in-from-bottom-1 motion-safe:duration-200">
  <header class="flex min-w-0 flex-wrap items-center justify-between gap-3 border-b border-border/50 px-4 py-3">
    <div class="flex min-w-0 flex-wrap items-baseline gap-x-3 gap-y-1">
      <h2 class="text-sm font-medium">{title} DNS</h2>
      <span class="break-all font-mono text-xs text-muted-foreground">{domain} · {recordType}</span>
    </div>
    <div class="grid min-h-6 min-w-28 place-items-end" title={updating ? `Updating ${pendingDomain} · ${pendingType}` : undefined}>
      <span class={`col-start-1 row-start-1 rounded-full border px-2.5 py-1 font-mono text-[11px] transition-opacity duration-150 motion-reduce:transition-none ${updating || !result ? "pointer-events-none opacity-0" : "opacity-100"} ${result?.status !== null && result?.status !== 0 ? "border-border/60 text-destructive" : "border-border/60 text-primary"}`} aria-hidden={updating || !result}>
        {result ? status : "SUCCESS"}{result && result.status !== null ? ` (${result.status})` : ""}
      </span>
      <span class={`col-start-1 row-start-1 inline-flex items-center gap-1.5 rounded-full px-2.5 py-1 text-[11px] text-muted-foreground transition-opacity duration-150 motion-reduce:transition-none ${updating ? "opacity-100" : "pointer-events-none opacity-0"}`} aria-hidden={!updating}>
        <Spinner class="size-3 border-current/25 border-t-current motion-reduce:animate-none" /> Updating…
        <span class="sr-only">{pendingDomain} · {pendingType}. Previous response remains visible.</span>
      </span>
    </div>
  </header>

  {#if loading && !result && !error}
    <div role="status" class="flex min-h-44 items-center justify-center gap-3 px-4 py-5 text-sm text-muted-foreground">
      <Spinner class="size-4 border-current/25 border-t-current text-primary motion-reduce:animate-none" /> Resolving with {title === "Core" ? "the active core" : "macOS"}…
    </div>
  {:else if error}
    <p role="alert" class="break-words px-4 py-4 text-sm text-destructive">{error}</p>
  {:else if result}
    <div class="grid min-w-0 grid-cols-1 xl:grid-cols-[minmax(0,1fr)_16rem]">
      <div class="min-w-0">
        <div class="flex min-h-12 items-center justify-between gap-3 px-4 py-1.5">
          <h3 class="text-xs font-medium text-muted-foreground">
            Records <span class="ml-2 font-normal tabular-nums opacity-60">{records.length}</span>
          </h3>
          {#if records.length > 0}
            <Button variant="ghost" size="sm" class="h-10 w-32 justify-center gap-1.5 rounded-full px-3 text-xs text-muted-foreground transition-[background-color,color,box-shadow]" onclick={copyRecords}>
              <HugeiconsIcon icon={copied ? Tick01Icon : Copy01Icon} class="size-3.5" />
              {copied ? "Copied" : "Copy records"}
            </Button>
          {/if}
        </div>
        {#if !hasAnswers}
          <p class="border-t border-border/40 px-4 py-4 text-sm leading-relaxed text-muted-foreground">
            {#if result.status === 3}
              This domain does not exist (NXDOMAIN).
            {:else if result.status === null || result.status === 0}
              No {result.record_type || "requested"} answer records were returned.
            {:else}
              The resolver returned {status} without answer records.
            {/if}
          </p>
        {/if}
        {#if records.length > 0}
          <div class="min-w-0 overflow-x-auto">
            <table class="w-full min-w-[34rem] border-collapse text-left text-xs">
              <thead class="border-y border-border/40 text-[11px] text-muted-foreground">
                <tr><th class="w-20 px-4 py-2 font-medium">Type</th><th class="w-44 px-3 py-2 font-medium">Name</th><th class="px-3 py-2 font-medium">Data</th><th class="w-20 px-4 py-2 text-right font-medium">TTL</th></tr>
              </thead>
              {#each groups as group (group.section)}
                <tbody class="divide-y divide-border/30">
                  <tr class="border-y border-border/30 bg-muted/15"><th colspan="4" class="px-4 py-2 text-[10px] font-medium uppercase tracking-wide text-muted-foreground">{group.section}</th></tr>
                  {#each group.records as record, index (`${index}:${record.name}:${record.record_type}:${record.data}`)}
                    <tr>
                      <td class="px-4 py-3 align-top font-mono text-muted-foreground">{record.record_type}</td>
                      <td class="break-all px-3 py-3 align-top font-mono text-muted-foreground">{record.name || domain}</td>
                      <td class="break-all px-3 py-3 align-top font-mono text-sm text-foreground">
                        {record.data}
                        {#if isFakeIp(record.data)}<span class="ml-2 inline-block rounded-full bg-primary/10 px-2 py-0.5 font-sans text-[10px] font-medium text-primary">FakeIP</span>{/if}
                      </td>
                      <td class="px-4 py-3 text-right align-top font-mono tabular-nums text-muted-foreground">{record.ttl === null ? "—" : `${record.ttl}s`}</td>
                    </tr>
                  {/each}
                </tbody>
              {/each}
            </table>
          </div>
        {/if}
        {#if hasFakeIp}
          <p class="border-t border-border/40 bg-muted/15 px-4 py-3 text-xs leading-relaxed text-muted-foreground">
            FakeIP addresses in 198.18.0.0/15 are virtual routing addresses, rather than public destination IPs.
          </p>
        {/if}
      </div>
      <aside aria-label={`${title} response details`} class="min-w-0 border-t border-border/50 bg-muted/10 p-4 xl:border-t-0 xl:border-l">
        <dl class="space-y-3 text-xs">
          <div class="flex items-baseline justify-between gap-4"><dt class="text-muted-foreground">Resolver</dt><dd class="break-all text-right font-medium">{resolverLabel}</dd></div>
          <div class="flex items-baseline justify-between gap-4"><dt class="text-muted-foreground">Latency</dt><dd class="font-mono tabular-nums">{result.latency_ms} ms</dd></div>
          <div class="flex items-baseline justify-between gap-4"><dt class="text-muted-foreground">Record</dt><dd class="font-mono">{result.record_type || "Not reported"}</dd></div>
        </dl>
        <details class="mt-4 border-t border-border/40 pt-3 text-xs text-muted-foreground">
          <summary class="w-fit cursor-pointer rounded-full outline-none focus-visible:ring-2 focus-visible:ring-ring">Technical details</summary>
          <dl class="mt-3 space-y-3">
            {#each traceFields as entry (entry.label)}
              <div class="flex items-baseline justify-between gap-4"><dt class="shrink-0">{entry.label}</dt><dd class="break-all text-right font-mono text-foreground">{entry.value}</dd></div>
            {/each}
          </dl>
          {#if title === "Core" && !trace}
            <p class="mt-3 text-[11px] leading-relaxed">Runtime trace is unavailable for this response. No upstream or matched rule has been inferred from saved settings.</p>
          {:else if title === "Core" && !trace?.server}
            <p class="mt-3 text-[11px] leading-relaxed">The runtime did not report an upstream server for this response.</p>
          {:else if title === "System"}
            <p class="mt-3 text-[11px] leading-relaxed">System results come from macOS. DNS response codes, TTLs, cache state, and upstream details are not exposed.</p>
          {/if}
          {#if title === "Core" && trace && (trace.rule_index !== null || trace.outbound)}
            <p class="mt-3 text-[11px] leading-relaxed">
              {trace.rule_index !== null ? "Runtime rule positions include generated Hosts and FakeIP rules. " : ""}
              {trace.outbound ? "The configured outbound may be a policy that selects another proxy." : ""}
            </p>
          {/if}
        </details>
      </aside>
    </div>
  {/if}
</section>
