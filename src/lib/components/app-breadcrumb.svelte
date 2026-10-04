<script lang="ts">
  import * as Breadcrumb from "$lib/components/ui/breadcrumb/index.js";

  type Props = {
    pageTitle: string;
  };

  const { pageTitle }: Props = $props();
</script>

<header
  class="flex h-16 shrink-0 items-center gap-2"
  data-tauri-drag-region
>
  <div class="flex items-center gap-2 px-4">
    <Breadcrumb.Root>
      <Breadcrumb.List>
        <Breadcrumb.Item class="hidden md:block">
          <Breadcrumb.Link href="/">Zeytun</Breadcrumb.Link>
        </Breadcrumb.Item>
        <Breadcrumb.Separator class="hidden md:block" />
        <Breadcrumb.Item>
          <Breadcrumb.Page>{pageTitle}</Breadcrumb.Page>
        </Breadcrumb.Item>
      </Breadcrumb.List>
    </Breadcrumb.Root>
  </div>
</header>