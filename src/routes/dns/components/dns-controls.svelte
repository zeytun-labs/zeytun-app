<script lang="ts">
  import { onDestroy } from "svelte";
  import { Button } from "$lib/components/ui/button";
  import { Switch } from "$lib/components/ui/switch";
  import type { DnsConfig } from "$lib/core/types";
  import { dnsResolutionDetails, dnsPolicyWarnings } from "$lib/core/dns-policy";
  import { ArrowRight01Icon, Delete01Icon } from "@hugeicons/core-free-icons";
  import { HugeiconsIcon } from "@hugeicons/svelte";
  import LoadingIcon from "./loading-icon.svelte";
  import { flushDnsCache } from "$lib/core/api";
  import { profileStore } from "$lib/stores/profile.svelte";
  import { errorMessage } from "$lib/errors";
  import { toast } from "svelte-sonner";
  import { createDnsLoadingIndicator } from "$lib/core/dns-loading";

  let saving = $state(false);
  let flushing = $state(false);
  let showFlushing = $state(false);
  const flushIndicator = createDnsLoadingIndicator((visible) => { showFlushing = visible; });
  onDestroy(() => flushIndicator.dispose());
  let operationError = $state<string | null>(null);
  const dns = $derived(profileStore.profile?.dns);
  const fakeIp = $derived(dns?.fake_ip ?? false);
  const outbound = $derived(profileStore.outboundMode);
  const savedPolicy = $derived(profileStore.pendingRestart || !profileStore.isRunning);
  const resolution = $derived(dnsResolutionDetails(dns, outbound, Object.fromEntries([
    ...(profileStore.profile?.policies ?? []).map((policy) => [policy.tag, policy.name === "ROOT_POLICY" || policy.name === "root-policy" ? "Global Proxy" : policy.name || "Proxy policy"]),
    ...(profileStore.profile?.proxies ?? []).map((proxy) => [proxy.tag, proxy.title || "Proxy"]),
  ])));
  const warnings = $derived(dnsPolicyWarnings(
    dns, outbound,
    profileStore.profile?.policies.find((policy) => policy.tag === "root-policy")?.selected_member_tag,
  ));

  async function setFakeIp(enabled: boolean) {
    if (saving || enabled === fakeIp) return;
    await updateDns({ fake_ip: enabled });
  }

  async function updateDns(changes: Partial<DnsConfig>) {
    const profile = profileStore.profile;
    if (!profile) return;
    saving = true;
    operationError = null;
    try {
      await profileStore.updateProfile({
        id: profile.id,
        dns: { ...profile.dns, ...changes },
      });
    } catch (error) {
      operationError = errorMessage(error);
    } finally {
      saving = false;
    }
  }

  async function flushCache() {
    if (flushing || showFlushing || !profileStore.isRunning) return;
    flushing = true;
    flushIndicator.start();
    operationError = null;
    try {
      await flushDnsCache();
      toast.success("Core DNS and FakeIP caches cleared.");
    } catch (error) {
      operationError = errorMessage(error);
    } finally {
      flushing = false;
      flushIndicator.finish();
    }
  }
</script>

<div class="mx-2 overflow-hidden rounded-2xl border border-border/50 bg-card/25">
  <div class="flex items-center justify-between gap-8 border-b border-border/40 p-5">
    <div class="min-w-0 space-y-2">
      <label for="dns-fakeip" class="cursor-pointer text-sm font-medium">FakeIP</label>
      <p class="text-xs leading-relaxed text-muted-foreground">Return virtual addresses for A / AAAA queries so the Core can route connections by domain.</p>
      {#if fakeIp && outbound === "direct"}
        <p class="text-xs text-muted-foreground">Bypassed in Direct mode; your preference stays saved.</p>
      {/if}
      {#if saving}
        <p role="status" class="text-xs text-muted-foreground">Saving…</p>
      {:else if profileStore.pendingRestart}
        <p class="text-xs text-muted-foreground">Saved · Restart Core to apply changes</p>
      {:else if !profileStore.isRunning}
        <p class="text-xs text-muted-foreground">Saved · Core is stopped</p>
      {/if}
    </div>
    <Switch
      id="dns-fakeip"
      checked={fakeIp}
      onCheckedChange={setFakeIp}
      disabled={saving || profileStore.loading || !profileStore.profile}
    />
  </div>

  <div class="flex flex-col gap-4 p-5 sm:flex-row sm:items-center sm:justify-between sm:gap-8">
    <div class="min-w-0 space-y-2">
      <p class="text-sm font-medium">DNS cache</p>
      <p class="text-xs leading-relaxed text-muted-foreground">Clear DNS responses and FakeIP mappings in the active Core. The macOS DNS cache is unchanged.</p>
    </div>
    <Button
      variant="outline"
      size="sm"
      class={`h-9 w-fit shrink-0 gap-2 rounded-full px-3 text-xs transition-[background-color,color,box-shadow,opacity] ${flushing || showFlushing ? "disabled:opacity-100" : ""}`}
      onclick={flushCache}
      disabled={flushing || showFlushing || !profileStore.isRunning}
      aria-label="Flush Core Cache"
      aria-busy={flushing}
      title={profileStore.isRunning ? "Clear the active core’s DNS and FakeIP caches" : "Start the core to clear its cache"}
    >
      <LoadingIcon loading={showFlushing} icon={Delete01Icon} />
      Flush Core Cache
    </Button>
  </div>
</div>

{#each warnings as warning (warning)}
  <p role="note" class="mx-2 mt-3 px-1 text-xs leading-relaxed text-muted-foreground"><span class="font-medium">{savedPolicy ? "Saved DNS exception:" : "DNS exception:"}</span> {warning}</p>
{/each}
<section
  aria-labelledby="dns-resolution-heading"
  class="mx-2 mt-4 overflow-hidden rounded-2xl border border-border/50 bg-card/25"
>
  <div class="space-y-4 p-5">
    <div class="flex flex-wrap items-start justify-between gap-x-4 gap-y-2">
      <div class="min-w-0 space-y-1.5">
        <h2 id="dns-resolution-heading" class="text-sm font-medium">Resolution details</h2>
        <p class="text-xs leading-relaxed text-muted-foreground">
          {#if profileStore.pendingRestart}
            Saved configuration · Restart Core to apply these settings.
          {:else if !profileStore.isRunning}
            Saved configuration · Core is stopped.
          {:else}
            Queries use the first matching step in this order.
          {/if}
        </p>
      </div>
      <span class="shrink-0 rounded-full border border-border/50 bg-muted/30 px-2.5 py-1 text-[11px] font-medium text-muted-foreground">
        {savedPolicy ? "Saved settings" : "Current settings"}
      </span>
    </div>

    <ol
      aria-label={savedPolicy ? "Saved resolution order" : "Resolution order"}
      class="flex flex-wrap items-center gap-x-2 gap-y-2"
    >
      {#each resolution.steps as step, index (step)}
        <li class="flex min-w-0 max-w-full items-center gap-2">
          {#if index > 0}
            <HugeiconsIcon icon={ArrowRight01Icon} class="size-3.5 shrink-0 text-muted-foreground/50" aria-hidden="true" />
          {/if}
          <span
            class={`min-w-0 rounded-full border px-3 py-1.5 text-xs leading-4 ${index === resolution.steps.length - 1 ? "border-primary/20 bg-primary/5 font-medium text-foreground" : "border-border/40 bg-muted/20 text-muted-foreground"}`}
            title={step}
          >
            {index === resolution.steps.length - 1 ? "Final" : step}
          </span>
        </li>
      {/each}
    </ol>
  </div>

  <dl class="grid gap-5 border-t border-border/40 px-5 py-4 sm:grid-cols-2 sm:gap-6">
    <div class="min-w-0 space-y-1.5">
      <dt class="text-xs text-muted-foreground">Final resolver</dt>
      <dd class="min-w-0 space-y-1">
        <span class="block break-words text-sm font-medium">{resolution.resolver}</span>
        {#if resolution.address}
          <span class="block break-all font-mono text-xs leading-relaxed text-muted-foreground">{resolution.address}</span>
        {/if}
      </dd>
    </div>
    <div class="min-w-0 space-y-1.5">
      <dt class="text-xs text-muted-foreground">DNS route</dt>
      <dd class="break-words text-sm leading-relaxed">{resolution.route}</dd>
    </div>
  </dl>

  <div class="space-y-2 border-t border-border/40 bg-muted/10 px-5 py-3 text-xs leading-relaxed text-muted-foreground">
    {#if resolution.fakeIpBypassed}
      <p>FakeIP is bypassed in Direct mode; unmatched queries use real addresses from Final.</p>
    {/if}
    <p>Hosts and matching DNS rules take priority and can use their own resolver routes. Otherwise .local, .home.arpa, localhost, and single-label names use Local (System) before Final.</p>
  </div>
</section>
{#if operationError}
  <p role="alert" class="mx-2 mt-3 px-1 text-xs text-destructive">{operationError}</p>
{/if}
