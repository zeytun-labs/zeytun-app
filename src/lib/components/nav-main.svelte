<script lang="ts">
  import { page } from "$app/state";
  import * as Sidebar from "$lib/components/ui/sidebar/index.js";
  import { HugeiconsIcon } from "@hugeicons/svelte";

  let {
    data,
  }: {
    data: {
      title?: string;
      items?: {
        title: string;
        url: string;
        icon: any;
      }[];
    }[];
  } = $props();

  function isActive(url: string) {
    if (url === "#") {
      return false;
    }

    if (url === "/") {
      return page.url.pathname === "/";
    }

    return page.url.pathname === url || page.url.pathname.startsWith(`${url}/`);
  }
</script>

<Sidebar.Group>
  {#each data as mainItem (mainItem.title)}
    {#if mainItem.title}
      <Sidebar.GroupLabel>{mainItem.title}</Sidebar.GroupLabel>
    {/if}
    <Sidebar.Menu>
      {#each mainItem.items as item (item.title)}
        <Sidebar.MenuItem>
          <Sidebar.MenuButton isActive={isActive(item.url)}>
            {#snippet child({ props })}
              <a href={item.url} {...props}>
                <HugeiconsIcon icon={item.icon} />
                <!-- <item.icon /> -->
                <span>{item.title}</span>
              </a>
            {/snippet}
          </Sidebar.MenuButton>
        </Sidebar.MenuItem>
      {/each}
    </Sidebar.Menu>
  {/each}
</Sidebar.Group>
