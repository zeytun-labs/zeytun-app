<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import { profileStore } from "$lib/stores/profile.svelte";
  import { networkQualityTestStore } from "$lib/stores/network-quality-test.svelte";
  import { stunTestStore } from "$lib/stores/stun-test.svelte";
  import * as Card from "$lib/components/ui/card";
  import { Button } from "$lib/components/ui/button";
  import { Input } from "$lib/components/ui/input";
  import { Label } from "$lib/components/ui/label";
  import { Switch } from "$lib/components/ui/switch";
  import * as Select from "$lib/components/ui/select";
  import { HugeiconsIcon } from "@hugeicons/svelte";
  import { ZapIcon, DatabaseIcon } from "@hugeicons/core-free-icons";

  // --- Network Quality State ---
  let nqConfigUrl = $state("https://mensura.cdn-apple.com/api/v1/gm/config");
  let nqOutbound = $state("direct");
  let nqMaxRuntime = $state("20");
  let nqSerial = $state(false);
  let nqHttp3 = $state(false);

  // --- STUN Test State ---
  let stunServer = $state("stun.voipgate.com:3478");
  let stunOutbound = $state("direct");

  onMount(() => {
    // Reset stores on mount so we don't show stale results from background runs
    networkQualityTestStore.reset();
    stunTestStore.reset();
  });

  onDestroy(() => {
    networkQualityTestStore.reset();
    stunTestStore.reset();
  });

  const nqLoading = $derived(networkQualityTestStore.testState === "Testing");
  const stunLoading = $derived(stunTestStore.testState === "Testing");

  // Format bps to Mbps
  const formatMbps = (bps: number | undefined) =>
    bps ? (bps / 1_000_000).toFixed(2) : "0.00";

  // Outbound options from current profile
  const outbounds = $derived.by(() => {
    // No "block" — testing against a null outbound is meaningless.
    const builtin = [{ value: "direct", label: "Direct" }];
    if (!profileStore.profile) return builtin;

    const policies = profileStore.profile.policies
      .filter((p) => p.tag !== "root-policy")
      .map((p) => ({ value: p.tag, label: p.name || p.tag }));
    const proxies = profileStore.profile.proxies
      .filter((p) => p.enabled)
      .map((p) => ({ value: p.tag, label: p.title || p.tag }));

    return [...builtin, ...policies, ...proxies];
  });

  const getOutboundLabel = (tag: string) =>
    outbounds.find((o) => o.value === tag)?.label || tag;

  const runtimeOptions = [
    { value: "20", label: "20 Seconds" },
    { value: "30", label: "30 Seconds" },
    { value: "60", label: "60 Seconds" },
  ];
  const getRuntimeLabel = (val: string) =>
    runtimeOptions.find((o) => o.value === val)?.label || `${val} Seconds`;
</script>

<div class="flex-1 flex-col gap-6 w-full mx-auto overflow-y-auto pb-6 px-px pt-1 grid grid-cols-1">
  <!-- NETWORK QUALITY TEST -->
  <Card.Root class="bg-card/70 border-border/40 rounded-4xl py-4!">
    <Card.Header class="flex flex-row items-center justify-between border-b border-border/40 pb-4!">
      <div class="flex items-center gap-3">
        <div class="bg-primary/10 text-primary p-2 rounded-xl">
          <HugeiconsIcon icon={ZapIcon} class="size-5" />
        </div>
        <div class="flex flex-col gap-1">
          <Card.Title class="text-base font-semibold">Network Quality Test</Card.Title>
          <Card.Description class="text-xs">Test latency, download, and upload capacity.</Card.Description>
        </div>
      </div>
      
      {#if nqLoading}
        <Button variant="destructive" onclick={() => networkQualityTestStore.cancelTest()}>Cancel</Button>
      {:else}
        <Button 
          onclick={() => networkQualityTestStore.startTest({
            configUrl: nqConfigUrl,
            outboundTag: nqOutbound,
            serial: nqSerial,
            maxRuntimeSeconds: parseInt(nqMaxRuntime, 10),
            http3: nqHttp3
          })}
        >
          Start Test
        </Button>
      {/if}
    </Card.Header>

    <Card.Content class="flex flex-col gap-4 pb-2">
      <!-- Inputs -->
      <div class="grid grid-cols-1 md:grid-cols-2 gap-x-6 gap-y-4">
        <div class="flex flex-col gap-2">
          <Label for="nq-config" class="text-xs text-muted-foreground uppercase tracking-wide">Configuration URL</Label>
          <Input id="nq-config" bind:value={nqConfigUrl} disabled={nqLoading} />
        </div>

        <div class="flex flex-col gap-2">
          <Label class="text-xs text-muted-foreground uppercase tracking-wide">Outbound</Label>
          <Select.Root
            type="single"
            name="nq-outbound"
            bind:value={nqOutbound}
            disabled={nqLoading}
          >
            <Select.Trigger class="w-full">
              {getOutboundLabel(nqOutbound)}
            </Select.Trigger>
            <Select.Content>
              <Select.Group>
                {#each outbounds as opt (opt.value)}
                  <Select.Item value={opt.value} label={opt.label}>
                    {opt.label}
                  </Select.Item>
                {/each}
              </Select.Group>
            </Select.Content>
          </Select.Root>
        </div>

        <div class="flex flex-col gap-2">
          <Label class="text-xs text-muted-foreground uppercase tracking-wide">Max Runtime</Label>
          <Select.Root
            type="single"
            name="nq-runtime"
            bind:value={nqMaxRuntime}
            disabled={nqLoading}
          >
            <Select.Trigger class="w-full">
              {getRuntimeLabel(nqMaxRuntime)}
            </Select.Trigger>
            <Select.Content>
              <Select.Group>
                {#each runtimeOptions as opt (opt.value)}
                  <Select.Item value={opt.value} label={opt.label}>
                    {opt.label}
                  </Select.Item>
                {/each}
              </Select.Group>
            </Select.Content>
          </Select.Root>
        </div>

        <div class="flex items-center gap-6 mt-1 md:mt-0">
          <div class="flex items-center gap-2 mt-6">
            <Switch id="nq-serial" bind:checked={nqSerial} disabled={nqLoading} />
            <Label for="nq-serial" class="cursor-pointer">Serial</Label>
          </div>
          <div class="flex items-center gap-2 mt-6">
            <Switch id="nq-http3" bind:checked={nqHttp3} disabled={nqLoading} />
            <Label for="nq-http3" class="cursor-pointer">HTTP/3</Label>
          </div>
        </div>
      </div>

      <!-- Results -->
      {#if networkQualityTestStore.testState !== "Idle"}
        <div class="mt-2 bg-muted/50 rounded-2xl p-4 grid grid-cols-2 md:grid-cols-6 gap-4">
          <div class="flex flex-col gap-1 text-center">
            <span class="text-xs text-muted-foreground uppercase tracking-wide">Latency</span>
            <span class="font-mono text-sm">{networkQualityTestStore.testProgress?.idleLatencyMs ?? 0} ms</span>
          </div>
          <div class="flex flex-col gap-1 text-center">
            <span class="text-xs text-muted-foreground uppercase tracking-wide">Download</span>
            <span class="font-mono text-sm">{formatMbps(networkQualityTestStore.testProgress?.downloadCapacity)} Mbps</span>
          </div>
          <div class="flex flex-col gap-1 text-center">
            <span class="text-xs text-muted-foreground uppercase tracking-wide">Upload</span>
            <span class="font-mono text-sm">{formatMbps(networkQualityTestStore.testProgress?.uploadCapacity)} Mbps</span>
          </div>
          <div class="flex flex-col gap-1 text-center">
            <span class="text-xs text-muted-foreground uppercase tracking-wide">DL RPM</span>
            <span class="font-mono text-sm">{networkQualityTestStore.testProgress?.downloadRpm ?? 0}</span>
          </div>
          <div class="flex flex-col gap-1 text-center">
            <span class="text-xs text-muted-foreground uppercase tracking-wide">UL RPM</span>
            <span class="font-mono text-sm">{networkQualityTestStore.testProgress?.uploadRpm ?? 0}</span>
          </div>
          <div class="flex flex-col gap-1 text-center">
            <span class="text-xs text-muted-foreground uppercase tracking-wide">Elapsed</span>
            <span class="font-mono text-sm">{networkQualityTestStore.testProgress?.elapsedMs ?? 0} ms</span>
          </div>

          {#if networkQualityTestStore.testState === "Error" && networkQualityTestStore.testProgress?.error}
            <div class="col-span-2 md:col-span-6 mt-2 text-sm text-red-500 bg-red-500/10 p-2 rounded">
              Error: {networkQualityTestStore.testProgress.error}
            </div>
          {/if}
        </div>
      {/if}
    </Card.Content>
  </Card.Root>

  <!-- STUN TEST -->
  <Card.Root class="bg-card/70 border-border/40 rounded-4xl py-4!">
    <Card.Header class="flex flex-row items-center justify-between border-b border-border/40 pb-4!">
      <div class="flex items-center gap-3">
        <div class="bg-primary/10 text-primary p-2 rounded-lg">
          <HugeiconsIcon icon={DatabaseIcon} class="size-5" />
        </div>
        <div class="flex flex-col gap-1">
          <Card.Title class="text-base font-semibold">STUN Test</Card.Title>
          <Card.Description class="text-xs">Test NAT traversal and mapping behavior.</Card.Description>
        </div>
      </div>
      
      {#if stunLoading}
        <Button variant="destructive" onclick={() => stunTestStore.cancelTest()}>Cancel</Button>
      {:else}
        <Button 
          onclick={() => stunTestStore.startTest({
            server: stunServer,
            outboundTag: stunOutbound
          })}
        >
          Start Test
        </Button>
      {/if}
    </Card.Header>

    <Card.Content class="flex flex-col gap-4 pb-2">
      <!-- Inputs -->
      <div class="grid gap-x-6 gap-y-4">
        <div class="flex flex-col gap-2">
          <Label for="stun-server" class="text-xs text-muted-foreground uppercase tracking-wide">Server</Label>
          <Input id="stun-server" bind:value={stunServer} disabled={stunLoading} />
        </div>

        <div class="flex flex-col gap-2">
          <Label class="text-xs text-muted-foreground uppercase tracking-wide">Outbound</Label>
          <Select.Root
            type="single"
            name="stun-outbound"
            bind:value={stunOutbound}
            disabled={stunLoading}
          >
            <Select.Trigger class="w-full">
              {getOutboundLabel(stunOutbound)}
            </Select.Trigger>
            <Select.Content>
              <Select.Group>
                {#each outbounds as opt (opt.value)}
                  <Select.Item value={opt.value} label={opt.label}>
                    {opt.label}
                  </Select.Item>
                {/each}
              </Select.Group>
            </Select.Content>
          </Select.Root>
        </div>
      </div>

      <!-- Results -->
      {#if stunTestStore.testState !== "Idle"}
        <div class="mt-2 bg-muted/50 rounded-2xl p-4 grid grid-cols-1 md:grid-cols-4 gap-4">
          <div class="flex flex-col gap-1 text-center">
            <span class="text-xs text-muted-foreground uppercase tracking-wide">External Address</span>
            <span class="font-mono text-sm truncate" title={stunTestStore.testProgress?.externalAddr || "N/A"}>
              {stunTestStore.testProgress?.externalAddr || "N/A"}
            </span>
          </div>
          <div class="flex flex-col gap-1 text-center">
            <span class="text-xs text-muted-foreground uppercase tracking-wide">Latency</span>
            <span class="font-mono text-sm">{stunTestStore.testProgress?.latencyMs ?? 0} ms</span>
          </div>
          <div class="flex flex-col gap-1 text-center">
            <span class="text-xs text-muted-foreground uppercase tracking-wide">NAT Mapping</span>
            <span class="font-mono text-sm">{stunTestStore.testProgress?.natMapping ?? 0}</span>
          </div>
          <div class="flex flex-col gap-1 text-center">
            <span class="text-xs text-muted-foreground uppercase tracking-wide">NAT Filtering</span>
            <span class="font-mono text-sm">{stunTestStore.testProgress?.natFiltering ?? 0}</span>
          </div>

          {#if stunTestStore.testState === "Error" && stunTestStore.testProgress?.error}
            <div class="col-span-1 md:col-span-4 mt-2 text-sm text-red-500 bg-red-500/10 p-2 rounded">
              Error: {stunTestStore.testProgress.error}
            </div>
          {/if}
        </div>
      {/if}
    </Card.Content>
  </Card.Root>
</div>