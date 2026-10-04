<script lang="ts" module>
  import {
    Home04Icon,
    DashboardSquare01Icon,
    CommandLineIcon,
    ArrowDataTransferHorizontalIcon,
    Book02Icon,
    Settings03Icon,
    Cardiogram01Icon,
    TestTube01Icon,
    BorderFullIcon,
    AddressBookIcon,
  } from "@hugeicons/core-free-icons";

  type NavItem = {
    title: string;
    url?: string;
    onClick?: () => void;
    icon: typeof Home04Icon;
  };

  const navMain: NavItem[] = [
    { title: "Home", url: "/", icon: Home04Icon },
    {
      title: "Control Center",
      url: "/control-center",
      icon: DashboardSquare01Icon,
    },
    { title: "Process", url: "/process", icon: CommandLineIcon },
    { title: "Policy", url: "/policy", icon: ArrowDataTransferHorizontalIcon },
    { title: "Rule", url: "/rule", icon: Book02Icon },
    { title: "DNS", url: "/dns", icon: AddressBookIcon },
    { title: "Tools", url: "/tools", icon: TestTube01Icon },
    { title: "Logs", url: "/logs", icon: BorderFullIcon },
  ];

  const navSecondary: NavItem[] = [
    {
      title: "Traffic Monitor",
      url: "/inspector",
      icon: Cardiogram01Icon,
    },
    {
      title: "Settings",
      onClick: () => {
        document.dispatchEvent(new CustomEvent("open-settings"));
      },
      icon: Settings03Icon,
    },
  ];
</script>

<script lang="ts">
  import { page } from "$app/state";
  import { HugeiconsIcon } from "@hugeicons/svelte";
  import { cn } from "$lib/utils";
  import LogoIcon from "./logo-icon.svelte";
  import NavProfile from "./nav-profile.svelte";
  import { coreReadActiveConfig } from "$lib/core/api";
  import { updaterStore } from "$lib/stores/updater.svelte";
  import { toast } from "svelte-sonner";
  import { errorMessage } from "$lib/errors";

  function isActive(url?: string) {
    if (!url) return false;
    if (url === "/") return page.url.pathname === "/";
    return page.url.pathname === url || page.url.pathname.startsWith(`${url}/`);
  }

  // Sliding hover pill — one blob that translates behind the hovered item.
  type Highlight = { y: number; h: number; on: boolean };
  let mainHi = $state<Highlight>({ y: 0, h: 40, on: false });
  let secHi = $state<Highlight>({ y: 0, h: 40, on: false });

  function moveHighlight(
    e: PointerEvent & { currentTarget: EventTarget & HTMLElement },
    set: (h: Highlight) => void,
  ) {
    const el = e.currentTarget;
    set({ y: el.offsetTop, h: el.offsetHeight, on: true });
  }

  const itemClass = cn(
    "relative z-10 flex h-9.5 w-full items-center rounded-full text-sm",
    "text-muted-foreground outline-none transition-colors",
    "hover:text-accent-foreground",
    "focus-visible:ring-2 focus-visible:ring-ring",
    "data-active:bg-primary data-active:text-primary-foreground",
    "data-active:hover:bg-primary data-active:hover:text-primary-foreground",
  );

  const pillClass = cn(
    "group/pill pointer-events-auto relative flex w-12 flex-col gap-1 overflow-hidden rounded-[1.5rem] p-1",
    "border border-border/40 bg-popover/70 text-card-foreground shadow-lg",
    "backdrop-blur-xl backdrop-saturate-150",
    "transition-[width] duration-300 ease-out will-change-[width]",
    "hover:w-48",
  );

  const blobClass =
    "pointer-events-none absolute inset-s-1 inset-e-1 z-0 rounded-full bg-accent transition-[transform,height,opacity] duration-300 ease-out";

  // Secret debug feature: rapid click logo to copy zeytun-core config JSON
  let debugClickCount = 0;
  let debugClickTimer: ReturnType<typeof setTimeout>;

  async function handleLogoClick() {
    debugClickCount++;
    if (debugClickTimer) clearTimeout(debugClickTimer);
    
    if (debugClickCount >= 7) {
      debugClickCount = 0;
      try {
        const configStr = await coreReadActiveConfig();
        await navigator.clipboard.writeText(configStr);
        toast.success("Debug: Sing-box config JSON copied to clipboard!");
      } catch (err) {
        toast.error("Failed to read core config", {
          description: errorMessage(err),
        });
        console.error(err);
      }
    } else {
      debugClickTimer = setTimeout(() => {
        debugClickCount = 0;
      }, 400); // 400ms window between clicks
    }
  }
</script>

{#snippet navLink(
  item: NavItem,
  onEnter: (
    e: PointerEvent & { currentTarget: EventTarget & HTMLElement },
  ) => void,
)}
  {#if item.onClick}
    <button
      type="button"
      data-active={isActive(item.url) || undefined}
      class={itemClass}
      aria-label={item.title === "Settings" && updaterStore.available
        ? `Settings — update ${updaterStore.ready ? "ready" : "available"}`
        : item.title}
      onclick={item.onClick}
      onpointerenter={onEnter}
    >
      <span class="relative flex size-9 shrink-0 items-center justify-center">
        <HugeiconsIcon icon={item.icon} class="size-5!" />
        {#if item.title === "Settings" && updaterStore.available}
          <span
            class="bg-primary ring-popover absolute end-1.5 top-1.5 size-2 rounded-full ring-2"
            aria-hidden="true"
          ></span>
        {/if}
      </span>
      <span
        class="truncate pr-2.5 opacity-0 transition-opacity duration-200 group-hover/pill:opacity-100"
      >
        {item.title === "Settings" && updaterStore.available
          ? updaterStore.ready
            ? "Update ready"
            : "Update available"
          : item.title}
      </span>
    </button>
  {:else}
    <a
      href={item.url}
      data-active={isActive(item.url) || undefined}
      class={itemClass}
      onpointerenter={onEnter}
    >
      <span class="flex size-9 shrink-0 items-center justify-center">
        <HugeiconsIcon icon={item.icon} class="size-5!" />
      </span>
      <span
        class="truncate pr-2.5 opacity-0 transition-opacity duration-200 group-hover/pill:opacity-100"
      >
        {item.title}
      </span>
    </a>
  {/if}
{/snippet}

<aside
  class="pointer-events-none fixed inset-y-0 inset-s-6 z-50 flex w-12 flex-col items-start pt-12 pb-8"
  aria-label="Main navigation"
>
  <div
    class="pointer-events-auto mb-4 flex size-12 shrink-0 items-center justify-center cursor-default"
    role="button"
    tabindex="0"
    onclick={handleLogoClick}
    onkeydown={(e) => e.key === "Enter" && handleLogoClick()}
  >
    <LogoIcon class="size-9! text-primary drop-shadow-sm" />
  </div>

  <nav
    class={pillClass}
    aria-label="Primary"
    onpointerleave={() => (mainHi = { ...mainHi, on: false })}
  >
    <div
      class={blobClass}
      style="transform: translateY({mainHi.y - 2}px); height: {mainHi.h -
        2}px; opacity: {mainHi.on ? 1 : 0}"
      aria-hidden="true"
    ></div>
    {#each navMain as item (item.title)}
      {@render navLink(item, (e) => moveHighlight(e, (h) => (mainHi = h)))}
    {/each}
  </nav>

  <div class="min-h-8 flex-1" aria-hidden="true"></div>

  <nav
    class={cn(pillClass, "mb-4")}
    aria-label="Secondary"
    onpointerleave={() => (secHi = { ...secHi, on: false })}
  >
    <div
      class={blobClass}
      style="transform: translateY({secHi.y - 2}px); height: {secHi.h -
        2}px; opacity: {secHi.on ? 1 : 0}"
      aria-hidden="true"
    ></div>
    {#each navSecondary as item (item.title)}
      {@render navLink(item, (e) => moveHighlight(e, (h) => (secHi = h)))}
    {/each}
  </nav>

  <div
    class="pointer-events-auto flex size-12 shrink-0 items-center justify-center"
  >
    <NavProfile compact />
  </div>
</aside>