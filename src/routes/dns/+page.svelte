<script lang="ts">
  import { onMount } from "svelte";
  import Tabs from "$lib/components/ui/tabs/tabs.svelte";
  import TabsList from "$lib/components/ui/tabs/tabs-list.svelte";
  import TabsTrigger from "$lib/components/ui/tabs/tabs-trigger.svelte";
  import TabsContent from "$lib/components/ui/tabs/tabs-content.svelte";
  import { Button } from "$lib/components/ui/button";
  import * as InputGroup from "$lib/components/ui/input-group";
  import { HugeiconsIcon } from "@hugeicons/svelte";
  import {
    Add01Icon,
    Search01Icon,
    Delete01Icon,
  } from "@hugeicons/core-free-icons";
  import { profileStore } from "$lib/stores/profile.svelte";

  let activeTab = $state("settings");
  let triggerAddServer = $state<(() => void) | undefined>(undefined);
  let triggerAddRule = $state<(() => void) | undefined>(undefined);
  let triggerAddHost = $state<(() => void) | undefined>(undefined);
  let triggerDeleteHost = $state<(() => void) | undefined>(undefined);
  let selectedHostId = $state<string | null>(null);
  let searchRule = $state("");

  onMount(async () => {
    if (!profileStore.profile) {
      await profileStore.load();
    }
  });

  import DnsServersTab from "./components/dns-servers-tab.svelte";
  import DnsRulesTab from "./components/dns-rules-tab.svelte";
  import DnsLocalHostsTab from "./components/dns-local-hosts-tab.svelte";
  import DnsLookupTab from "./components/dns-lookup-tab.svelte";
  import DnsControls from "./components/dns-controls.svelte";
</script>

<div class="flex min-w-0 flex-1 flex-col gap-6">
  <Tabs bind:value={activeTab} class="min-w-0">
    <div class="flex flex-col">
      <div
        class="flex items-center justify-between w-full overflow-x-auto pb-1 gap-4"
      >
        <TabsList aria-label="DNS views" class="min-w-max" variant="primary">
          <TabsTrigger value="settings">Settings</TabsTrigger>
          <TabsTrigger value="servers">Servers</TabsTrigger>
          <TabsTrigger value="rules">DNS Rules</TabsTrigger>
          <TabsTrigger value="hosts">Local Hosts</TabsTrigger>
          <TabsTrigger value="lookup">Lookup Tool</TabsTrigger>
        </TabsList>

        <div class="shrink-0 flex items-center pr-1 gap-1.5">
          {#if activeTab === "rules"}
            <div class="hidden sm:block w-48 mr-2">
              <InputGroup.Root variant="backless" class="h-9">
                <InputGroup.Input
                  placeholder="Search rules..."
                  bind:value={searchRule}
                />
                <InputGroup.Addon>
                  <HugeiconsIcon icon={Search01Icon} class="size-4" />
                </InputGroup.Addon>
              </InputGroup.Root>
            </div>
            <Button
              variant="outline"
              class="bg-card/70! text-muted-foreground hover:text-foreground"
              onclick={() => triggerAddRule?.()}
            >
              <HugeiconsIcon icon={Add01Icon} />
              Add Rule
            </Button>
          {/if}
          {#if activeTab === "hosts"}
            {#if selectedHostId}
              <Button
                size="icon"
                variant="destructive"
                class="animate-in fade-in zoom-in-90 duration-200"
                disabled={!selectedHostId}
                onclick={() => triggerDeleteHost?.()}
              >
                <HugeiconsIcon icon={Delete01Icon} class="size-4" />
                <span class="sr-only">Delete selected host</span>
              </Button>
            {/if}
            <Button
              variant="outline"
              class="bg-card/70! text-muted-foreground hover:text-foreground"
              onclick={() => triggerAddHost?.()}
            >
              <HugeiconsIcon icon={Add01Icon} />
              Add Host
            </Button>
          {/if}
          {#if activeTab === "servers"}
            <Button
              variant="outline"
              class="bg-card/70! text-muted-foreground hover:text-foreground"
              onclick={() => triggerAddServer?.()}
            >
              <HugeiconsIcon icon={Add01Icon} />
              Add Server
            </Button>
          {/if}
        </div>
      </div>

      {#key activeTab}
        <div class="text-[13px] text-muted-foreground/80 px-2 motion-safe:animate-in motion-safe:fade-in motion-safe:duration-150">
          {#if activeTab === "settings"}
            Manage FakeIP and the Core DNS cache.
          {:else if activeTab === "servers"}
            Configure resolvers and the outbound each uses. Use a row’s menu to edit or delete it, or double-click to edit.
          {:else if activeTab === "rules"}
            Route matching domains through a configured DNS server or block them.
          {:else if activeTab === "hosts"}
            Resolve a domain to a fixed IPv4 or IPv6 address before querying
            upstream DNS.
          {:else if activeTab === "lookup"}
            Look up a domain using your DNS servers and rules.
          {/if}
        </div>
      {/key}
    </div>

    <TabsContent value="settings" class="min-w-0">
      <DnsControls />
    </TabsContent>

    <TabsContent value="servers" class="min-w-0">
      <DnsServersTab bind:triggerAdd={triggerAddServer} />
    </TabsContent>

    <TabsContent value="rules" class="min-w-0">
      <DnsRulesTab bind:triggerAdd={triggerAddRule} bind:search={searchRule} />
    </TabsContent>

    <TabsContent value="hosts" class="min-w-0">
        <DnsLocalHostsTab
          bind:triggerAdd={triggerAddHost}
          bind:triggerDelete={triggerDeleteHost}
          bind:selectedId={selectedHostId}
        />
    </TabsContent>

    <TabsContent value="lookup" class="min-w-0">
        <DnsLookupTab />
    </TabsContent>
  </Tabs>
</div>
