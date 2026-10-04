<script lang="ts">
  import { page } from "$app/state";
  import * as Sidebar from "$lib/components/ui/sidebar/index.js";
  import type { ComponentProps } from "svelte";
  import { HugeiconsIcon } from "@hugeicons/svelte";

  let {
    ref = $bindable(null),
    items,
    ...restProps
  }: {
    items: {
      title: string;
      url?: string;
      onClick?: () => any;
      icon: any;
    }[];
  } & ComponentProps<typeof Sidebar.Group> = $props();

  function isActive(url: string) {
    if (url === "#") {
      return false;
    }

    return page.url.pathname === url || page.url.pathname.startsWith(`${url}/`);
  }
</script>

<Sidebar.Group bind:ref {...restProps}>
  <Sidebar.GroupContent>
    <Sidebar.Menu>
      {#each items as item (item.title)}
        <Sidebar.MenuItem>
          <Sidebar.MenuButton isActive={isActive(item.url || "none")}>
            {#snippet child({ props })}
              {#if item.url}
                <a href={item.url} {...props}>
                  <HugeiconsIcon icon={item.icon} />
                  <span>{item.title}</span>
                </a>
              {:else}
                <button {...props} onclick={item.onClick}>
                  <HugeiconsIcon icon={item.icon} />
                  <span>{item.title}</span>
                </button>
              {/if}
            {/snippet}
          </Sidebar.MenuButton>
        </Sidebar.MenuItem>
      {/each}
    </Sidebar.Menu>
  </Sidebar.GroupContent>
</Sidebar.Group>
