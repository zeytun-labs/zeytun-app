<script lang="ts">
  import { onMount } from "svelte";
  import { toast } from "svelte-sonner";
  import Tabs from "$lib/components/ui/tabs/tabs.svelte";
  import TabsList from "$lib/components/ui/tabs/tabs-list.svelte";
  import TabsTrigger from "$lib/components/ui/tabs/tabs-trigger.svelte";
  import TabsContent from "$lib/components/ui/tabs/tabs-content.svelte";
  import * as Card from "$lib/components/ui/card";
  import { Switch } from "$lib/components/ui/switch";
  import { Button } from "$lib/components/ui/button";
  import * as Tooltip from "$lib/components/ui/tooltip";
  import * as InputGroup from "$lib/components/ui/input-group";
  import { Spinner } from "$lib/components/ui/spinner";
  import { HugeiconsIcon } from "@hugeicons/svelte";
  import {
    InformationCircleIcon,
    GlobalSearchIcon,
    CleanIcon,
    Add01Icon,
    Search01Icon,
    Delete01Icon,
  } from "@hugeicons/core-free-icons";
  import { profileStore } from "$lib/stores/profile.svelte";

  let activeTab = $state("servers");
  let triggerAddServer = $state<(() => void) | undefined>(undefined);
  let triggerDeleteServer = $state<(() => void) | undefined>(undefined);
  let selectedServerId = $state<string | null>(null);
  let triggerAddRule = $state<(() => void) | undefined>(undefined);
  let triggerDeleteRule = $state<(() => void) | undefined>(undefined);
  let selectedRuleId = $state<string | null>(null);
  let triggerAddHost = $state<(() => void) | undefined>(undefined);
  let triggerDeleteHost = $state<(() => void) | undefined>(undefined);
  let selectedHostId = $state<string | null>(null);
  let searchRule = $state("");
  let isToggling = $state(false);

  onMount(async () => {
    if (!profileStore.profile) {
      await profileStore.load();
    }
  });

  import DnsServersTab from "./components/dns-servers-tab.svelte";
  import DnsRulesTab from "./components/dns-rules-tab.svelte";
  import DnsLocalHostsTab from "./components/dns-local-hosts-tab.svelte";
  import ScrollArea from "$lib/components/ui/scroll-area/scroll-area.svelte";
</script>

<div class="flex min-w-0 flex-1 flex-col gap-6">
  <Tabs bind:value={activeTab} class="min-w-0">
    <div class="flex flex-col">
      <div
        class="flex items-center justify-between w-full overflow-x-auto pb-1 gap-4"
      >
        <TabsList aria-label="DNS views" class="min-w-max" variant="primary">
          <TabsTrigger value="servers">Servers</TabsTrigger>
          <TabsTrigger value="rules">DNS Rules</TabsTrigger>
          <TabsTrigger value="hosts">Local Hosts</TabsTrigger>
          <!-- <TabsTrigger value="lookup">Lookup Tool</TabsTrigger> -->
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
            {#if selectedRuleId}
              <Button
                size="icon"
                variant="destructive"
                class="animate-in fade-in zoom-in-90 duration-200"
                disabled={!selectedRuleId}
                onclick={() => triggerDeleteRule?.()}
              >
                <HugeiconsIcon icon={Delete01Icon} />
                <span class="sr-only">Delete selected rule</span>
              </Button>
            {/if}
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
            {#if selectedServerId}
              <Button
                size="icon"
                variant="destructive"
                class="animate-in fade-in zoom-in-90 duration-200"
                onclick={() => triggerDeleteServer?.()}
              >
                <HugeiconsIcon icon={Delete01Icon} />
                <span class="sr-only">Delete selected server</span>
              </Button>
            {/if}
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

      <div class="text-[13px] text-muted-foreground/80 px-2">
        {#if activeTab === "servers"}
          List of all DNS servers used for resolution. Assign outbounds to
          control routing and whether it is a Local or Remote DNS.
        {:else if activeTab === "rules"}
          Route matching domains through a configured DNS server or block them.
        {:else if activeTab === "hosts"}
          Resolve a domain to a fixed IPv4 or IPv6 address before querying
          upstream DNS.
        {:else if activeTab === "lookup"}
          Query configured DNS servers to test routing and resolution behavior.
        {/if}
      </div>
    </div>

    <TabsContent value="servers" class="min-w-0">
      <ScrollArea class="h-[calc(100vh-155px)] **:data-[slot=scroll-area-scrollbar]:hidden">
        <DnsServersTab
          bind:triggerAdd={triggerAddServer}
          bind:triggerDelete={triggerDeleteServer}
          bind:selectedId={selectedServerId}
        />
      </ScrollArea>
    </TabsContent>

    <TabsContent value="rules" class="min-w-0">
      <ScrollArea class="h-[calc(100vh-155px)] **:data-[slot=scroll-area-scrollbar]:hidden">
        <DnsRulesTab
          bind:triggerAdd={triggerAddRule}
          bind:triggerDelete={triggerDeleteRule}
          bind:selectedId={selectedRuleId}
          bind:search={searchRule}
        />
      </ScrollArea>
    </TabsContent>

    <TabsContent value="hosts" class="min-w-0">
      <ScrollArea class="h-[calc(100vh-155px)] **:data-[slot=scroll-area-scrollbar]:hidden">
        <DnsLocalHostsTab
          bind:triggerAdd={triggerAddHost}
          bind:triggerDelete={triggerDeleteHost}
          bind:selectedId={selectedHostId}
        />
      </ScrollArea>
    </TabsContent>

    <!-- <TabsContent value="lookup" class="min-w-0">
      <DnsLookupTab />
    </TabsContent> -->
  </Tabs>
</div>
