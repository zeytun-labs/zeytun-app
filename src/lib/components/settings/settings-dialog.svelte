<script lang="ts">
  import * as Dialog from "$lib/components/ui/dialog/index.js";
  import { Button } from "$lib/components/ui/button/index.js";
  import { ScrollArea } from "$lib/components/ui/scroll-area/index.js";
  import { HugeiconsIcon } from "@hugeicons/svelte";
  import {
    Settings01Icon,
    EarthIcon,
    ServerStack01Icon,
    CpuSettingsIcon,
    InformationCircleIcon,
    ReloadIcon,
  } from "@hugeicons/core-free-icons";
  import { untrack } from "svelte";
  import { slide } from "svelte/transition";
  import { toast } from "svelte-sonner";
  import { cn } from "$lib/utils.js";
  import { coreRestart } from "$lib/core/api";
  import { updaterStore } from "$lib/stores/updater.svelte";
  import { slidingHighlight } from "$lib/actions/sliding-highlight";

  import { SettingsDraft } from "./settings-draft.svelte";
  import Spinner from "$lib/components/ui/spinner/spinner.svelte";
  import GeneralSection from "./sections/general-section.svelte";
  import NetworkSection from "./sections/network-section.svelte";
  import DnsSection from "./sections/dns-section.svelte";
  import ConnectivitySection from "./sections/connectivity-section.svelte";
  import AboutSection from "./sections/about-section.svelte";
  import { errorMessage } from "$lib/errors";

  let { open = $bindable(false), initialTab = "general" } = $props<{
    open: boolean;
    initialTab?: string;
  }>();

  let activeTab = $state("general");
  let draft = $state<SettingsDraft>(new SettingsDraft());

  const categories = [
    { id: "general", label: "General", icon: Settings01Icon },
    { id: "network", label: "Network", icon: EarthIcon },
    { id: "dns", label: "DNS", icon: ServerStack01Icon },
    // Holds test endpoints, not connection settings — "Connectivity" read as a
    // synonym of "Network" next to it.
    { id: "connectivity", label: "Diagnostics", icon: CpuSettingsIcon },
    { id: "about", label: "About", icon: InformationCircleIcon },
  ];

  // Same active language as the main app rail: a primary pill that slides between
  // items. The blob IS the indicator (design-system rule), so the buttons carry no
  // `data-active:bg-*` of their own — only text colour, which flips on
  // `data-blob-on` as the blob arrives. Plus the focus ring: these are raw
  // buttons, so nothing supplies it for us.
  const categoryClass = cn(
    "group relative z-10 flex w-full items-center gap-2 rounded-full border border-transparent bg-clip-padding",
    "px-3 py-2 text-left text-sm outline-none transition-colors",
    "text-sidebar-foreground/60 hover:text-sidebar-accent-foreground",
    "focus-visible:border-ring focus-visible:ring-ring/30 focus-visible:ring-3",
    "data-active:font-medium data-active:text-sidebar-foreground",
    // `!` because a plain attribute variant ties on specificity with
    // `data-active:`/`hover:` above, and CSS source order would then decide.
    "data-[blob-on]:text-primary-foreground! data-[blob-on]:hover:text-primary-foreground!",
  );

  // Sliding pill: one action drives both hover (faint) and selected (primary).
  let navRef = $state<HTMLElement | null>(null);
  $effect(() => {
    if (!navRef) return;
    return slidingHighlight(navRef, {
      itemSelector: '[data-slot="settings-item"]',
      radius: "rounded-full",
      blobClass: "bg-primary",
      hoverBlobClass: "bg-sidebar-accent",
    }).destroy;
  });

  // Both footers must land on the same baseline: the sidebar's Restart Core rule
  // and the Save/Cancel rule read as one line broken by the column divider, so
  // the vertical metrics live here once instead of being matched by eye.
  const footerRow =
    "flex shrink-0 items-center border-t border-border/40 py-4";

  // Re-sync drafts and reset active tab every time the dialog opens.
  $effect(() => {
    if (open) {
      untrack(() => {
        activeTab = initialTab;
        draft.reset();
      });
    }
  });

  let isRestarting = $state(false);

  async function handleRestartCore() {
    if (isRestarting) return;
    isRestarting = true;
    try {
      await coreRestart();
      toast.success("Core restarted");
    } catch (e) {
      toast.error("Failed to restart core", { description: errorMessage(e) });
    } finally {
      isRestarting = false;
    }
  }

  async function handleSave() {
    if (await draft.apply()) {
      open = false;
    }
  }

  function handleCancel() {
    draft.reset();
    open = false;
  }
</script>

<Dialog.Root bind:open>
  <Dialog.Content
    onOpenAutoFocus={(e) => e.preventDefault()}
    class="flex h-[80vh] w-[56rem] max-w-[calc(100%-2rem)] flex-row gap-0 overflow-hidden rounded-4xl bg-background p-0 sm:max-w-[calc(100%-2rem)]"
  >
    <!-- Sidebar -->
    <div
      class="flex h-full w-52 shrink-0 flex-col justify-between border-r border-border/40 bg-sidebar"
    >
      <ScrollArea class="flex-1">
        <div
          bind:this={navRef}
          class="flex flex-col gap-1 px-3 pt-4 pb-3"
        >
          <Dialog.Title
            class="px-3 pb-3 text-sm font-semibold text-sidebar-foreground/70"
          >
            Settings
          </Dialog.Title>
          <Dialog.Description class="sr-only">
            Zeytun application settings
          </Dialog.Description>
          {#each categories as category (category.id)}
            <button
              type="button"
              data-slot="settings-item"
              data-active={activeTab === category.id || undefined}
              class={categoryClass}
              onclick={() => (activeTab = category.id)}
            >
              <HugeiconsIcon icon={category.icon} class="size-4 shrink-0" />
              {category.label}
              {#if category.id === "about" && updaterStore.available}
                <span
                  class="bg-primary ring-sidebar group-data-[blob-on]:bg-primary-foreground ms-auto size-2 rounded-full ring-2"
                  aria-label={updaterStore.ready
                    ? "Update ready"
                    : "Update available"}
                ></span>
              {/if}
            </button>
          {/each}
        </div>
      </ScrollArea>
      <div class={cn(footerRow, "px-3")}>
        <Button
          variant="ghost"
          class="w-full justify-start gap-2 rounded-full"
          onclick={handleRestartCore}
          disabled={isRestarting}
        >
          {#if isRestarting}
            <Spinner
              class="size-4 border-sidebar-foreground/30 border-t-sidebar-foreground"
            />
          {:else}
            <HugeiconsIcon icon={ReloadIcon} class="size-4 shrink-0" />
          {/if}
          Restart Core
        </Button>
      </div>
    </div>

    <!-- Content column: scroll area + footer are flex siblings, so the footer
         never floats over the text it is anchored below. -->
    <div class="flex min-w-0 flex-1 flex-col">
      <ScrollArea class="min-h-0 flex-1 bg-background">
        <div class="px-16 py-8">
          {#if activeTab === "general"}
            <GeneralSection {draft} />
          {:else if activeTab === "network"}
            <NetworkSection {draft} />
          {:else if activeTab === "dns"}
            <DnsSection {draft} />
          {:else if activeTab === "connectivity"}
            <ConnectivitySection {draft} />
          {:else}
            <AboutSection />
          {/if}
        </div>
      </ScrollArea>

      {#if draft.hasChanges}
        <div
          class={cn(footerRow, "justify-end gap-3 px-8")}
          transition:slide={{ duration: 200 }}
        >
          <Button variant="outline" onclick={handleCancel} disabled={draft.saving}>
            Cancel
          </Button>
          <Button onclick={handleSave} disabled={draft.saving}>
            {#if draft.saving}
              <Spinner class="border-background/30 border-t-background" />
            {:else}
              Save
            {/if}
          </Button>
        </div>
      {/if}
    </div>
  </Dialog.Content>
</Dialog.Root>
