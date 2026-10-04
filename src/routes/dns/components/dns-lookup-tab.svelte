<script lang="ts">
  import { toast } from "svelte-sonner";
  import * as Card from "$lib/components/ui/card";
  import { Input } from "$lib/components/ui/input/index.js";
  import { Button } from "$lib/components/ui/button/index.js";
  import { Spinner } from "$lib/components/ui/spinner";
  import { HugeiconsIcon } from "@hugeicons/svelte";
  import {
    Activity01Icon,
    Copy01Icon,
    GlobalSearchIcon,
    Search01Icon,
    ServerStack01Icon,
    Target01Icon,
    Tick01Icon,
  } from "@hugeicons/core-free-icons";
  import { testDnsLookup, type DnsLookupResult } from "$lib/core/api";
  import { errorMessage } from "$lib/errors";

  let domain = $state("youtube.com");
  let isLoading = $state(false);
  let lookupResult = $state<DnsLookupResult | null>(null);
  let queriedDomain = $state("");
  let lookupError = $state<string | null>(null);
  let copied = $state(false);

  const statusNames: Record<number, string> = {
    0: "SUCCESS",
    1: "FORMERR",
    2: "SERVFAIL",
    3: "NXDOMAIN",
    4: "NOTIMP",
    5: "REFUSED",
    6: "YXDOMAIN",
    7: "YXRRSET",
    8: "NXRRSET",
    9: "NOTAUTH",
    10: "NOTZONE",
  };

  function statusLabel(status: number): string {
    return statusNames[status] ?? "DNS ERROR";
  }

  function hasAddresses(result: DnsLookupResult): boolean {
    return result.resolved_ips.length > 0;
  }

  function resultTone(result: DnsLookupResult): string {
    if (result.status !== 0) {
      return "border-red-500/20 bg-red-500/10 text-red-500";
    }
    if (!hasAddresses(result)) {
      return "border-amber-500/20 bg-amber-500/10 text-amber-500";
    }
    return "border-emerald-500/20 bg-emerald-500/10 text-emerald-500";
  }

  function resultDot(result: DnsLookupResult): string {
    if (result.status !== 0) return "bg-red-500";
    return hasAddresses(result) ? "bg-emerald-500" : "bg-amber-500";
  }

  async function handleLookup() {
    const query = domain.trim();
    if (!query) {
      toast.error("Please enter a valid domain.");
      return;
    }

    isLoading = true;
    lookupResult = null;
    lookupError = null;
    queriedDomain = query;

    try {
      const result = await testDnsLookup(query);
      lookupResult = result;
      if (result.status === 0 && result.resolved_ips.length > 0) {
        toast.success(`DNS lookup completed for ${query}.`);
      }
    } catch (error) {
      console.error("DNS lookup error:", error);
      lookupError = `DNS lookup failed: ${errorMessage(error)}`;
      toast.error(lookupError);
    } finally {
      isLoading = false;
    }
  }

  function handleKeydown(event: KeyboardEvent) {
    if (event.key === "Enter") void handleLookup();
  }

  async function copyAddresses() {
    if (!lookupResult || lookupResult.resolved_ips.length === 0) return;
    try {
      await navigator.clipboard.writeText(lookupResult.resolved_ips.join("\n"));
      copied = true;
      toast.success(
        lookupResult.resolved_ips.length === 1
          ? "IP copied to clipboard."
          : "IP addresses copied to clipboard.",
      );
      setTimeout(() => (copied = false), 2000);
    } catch {
      toast.error("Failed to copy IP addresses.");
    }
  }
</script>

<div class="mx-auto flex w-full max-w-4xl min-w-0 flex-col gap-6">
  <Card.Root class="bg-card/70 border-border/40 rounded-3xl p-5 shadow-sm sm:p-6">
    <Card.Header class="p-0 pb-4">
      <div class="flex items-center gap-2">
        <HugeiconsIcon icon={GlobalSearchIcon} class="size-5 text-primary" />
        <Card.Title class="text-lg font-semibold">DNS Lookup Tool</Card.Title>
      </div>
      <Card.Description class="pl-7 text-xs text-muted-foreground">
        Test DNS resolution and inspect the response returned by the active core.
      </Card.Description>
    </Card.Header>

    <Card.Content class="p-0 pt-2">
      <div class="flex flex-col items-stretch gap-2.5 sm:flex-row sm:items-center">
        <div class="relative min-w-0 flex-1">
          <Input
            bind:value={domain}
            placeholder="e.g. youtube.com or api.github.com"
            class="h-10.5 rounded-2xl border-border/40 bg-muted/40 pl-4 pr-10 font-mono text-sm focus-visible:ring-primary/30"
            onkeydown={handleKeydown}
            disabled={isLoading}
          />
        </div>
        <Button
          class="h-10.5 w-full shrink-0 gap-2 rounded-2xl px-6 font-medium shadow-sm sm:w-auto"
          onclick={handleLookup}
          disabled={isLoading}
        >
          {#if isLoading}
            <Spinner class="size-4 border-white/30 border-t-white" />
            <span>Testing...</span>
          {:else}
            <HugeiconsIcon icon={Search01Icon} class="size-4" />
            <span>Test Lookup</span>
          {/if}
        </Button>
      </div>
    </Card.Content>
  </Card.Root>

  {#if isLoading}
    <Card.Root class="bg-card/70 border-border/40 flex flex-col items-center justify-center gap-3 rounded-3xl p-8 text-center shadow-sm">
      <Spinner class="size-7 text-primary" />
      <div class="flex flex-col gap-1">
        <p class="text-sm font-medium text-foreground">Resolving domain...</p>
        <p class="font-mono text-xs text-muted-foreground">{queriedDomain}</p>
      </div>
    </Card.Root>
  {:else if lookupError}
    <Card.Root class="rounded-3xl border-destructive/30 bg-destructive/10 p-5 shadow-sm">
      <p class="wrap-break-word text-sm text-destructive">{lookupError}</p>
    </Card.Root>
  {:else if lookupResult}
    <Card.Root class="bg-card/70 border-border/40 flex min-w-0 flex-col gap-5 rounded-3xl p-5 shadow-sm sm:p-6">
      <div class="flex min-w-0 flex-col gap-3 border-b border-border/30 pb-4 sm:flex-row sm:items-center sm:justify-between">
        <div class="flex min-w-0 items-center gap-2.5">
          <span class={`size-2.5 shrink-0 rounded-full ${resultDot(lookupResult)}`}></span>
          <span class="truncate font-mono text-base font-semibold text-foreground" title={queriedDomain}>
            {queriedDomain}
          </span>
        </div>
        <span class={`inline-flex w-fit items-center rounded-full border px-2.5 py-0.5 text-xs font-semibold ${resultTone(lookupResult)}`}>
          {statusLabel(lookupResult.status)} ({lookupResult.status})
        </span>
      </div>

      <div class="grid grid-cols-1 gap-3 md:grid-cols-3">
        <div class="flex min-w-0 flex-col gap-2 rounded-2xl border border-border/30 bg-background/60 p-4 transition-colors hover:bg-background/80 md:col-span-1">
          <div class="flex items-center justify-between gap-2">
            <span class="flex items-center gap-1.5 text-xs font-medium text-muted-foreground">
              <HugeiconsIcon icon={Target01Icon} class="size-3.5 text-emerald-500" />
              Resolved IPs
            </span>
            {#if lookupResult.resolved_ips.length > 0}
              <Button
                variant="ghost"
                size="icon-xs"
                class="text-muted-foreground hover:text-foreground"
                onclick={copyAddresses}
                title="Copy all resolved IP addresses"
              >
                {#if copied}
                  <HugeiconsIcon icon={Tick01Icon} class="size-3.5 text-emerald-500" />
                {:else}
                  <HugeiconsIcon icon={Copy01Icon} class="size-3.5" />
                {/if}
              </Button>
            {/if}
          </div>

          {#if lookupResult.resolved_ips.length > 0}
            <div class="flex min-w-0 flex-col gap-1.5">
              {#each lookupResult.resolved_ips as ip (ip)}
                <span class="break-all rounded-lg bg-muted/30 px-2 py-1 font-mono text-sm font-semibold text-foreground">
                  {ip}
                </span>
              {/each}
            </div>
          {:else if lookupResult.status === 3}
            <p class="text-sm text-muted-foreground">
              This domain does not exist (NXDOMAIN); no IP addresses were returned.
            </p>
          {:else if lookupResult.status === 0}
            <p class="text-sm text-muted-foreground">
              The lookup succeeded, but no {lookupResult.record_type || "requested"} records were returned.
            </p>
          {:else}
            <p class="text-sm text-muted-foreground">
              The resolver returned {statusLabel(lookupResult.status)} without any IP addresses.
            </p>
          {/if}
        </div>

        <div class="flex min-w-0 flex-col gap-1.5 rounded-2xl border border-border/30 bg-background/60 p-4 transition-colors hover:bg-background/80">
          <span class="flex items-center gap-1.5 text-xs font-medium text-muted-foreground">
            <HugeiconsIcon icon={ServerStack01Icon} class="size-3.5 text-sky-500" />
            DNS Server Used
          </span>
          <span class="wrap-break-word font-mono text-base font-bold tracking-tight text-foreground">
            {lookupResult.server_used || "Not reported"}
          </span>
        </div>

        <div class="flex flex-col gap-1.5 rounded-2xl border border-border/30 bg-background/60 p-4 transition-colors hover:bg-background/80">
          <span class="flex items-center gap-1.5 text-xs font-medium text-muted-foreground">
            <HugeiconsIcon icon={Activity01Icon} class="size-3.5 text-amber-500" />
            Query Latency
          </span>
          <span class="font-mono text-base font-bold tracking-tight text-foreground">
            {lookupResult.latency_ms} ms
          </span>
        </div>
      </div>

      <div class="flex flex-col gap-2 rounded-2xl border border-border/20 bg-muted/30 px-4 py-3 text-xs text-muted-foreground sm:flex-row sm:items-center sm:justify-between">
        <span>
          Record Type:
          <strong class="font-mono text-foreground">
            {lookupResult.record_type || "Not reported"}
          </strong>
        </span>
        <span>
          TTL:
          <strong class="font-mono text-foreground">
            {lookupResult.ttl === null ? "Not provided" : `${lookupResult.ttl}s`}
          </strong>
        </span>
      </div>
    </Card.Root>
  {/if}
</div>
