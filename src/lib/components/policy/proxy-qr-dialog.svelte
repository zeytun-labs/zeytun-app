<script lang="ts">
  import * as Dialog from "$lib/components/ui/dialog/index.js";
  import { Button } from "$lib/components/ui/button/index.js";
  import { writeText } from "@tauri-apps/plugin-clipboard-manager";
  import { toast } from "svelte-sonner";
  import { renderSVG } from "uqr";
  import { errorMessage } from "$lib/errors";

  interface Props {
    open?: boolean;
    link?: string;
    title?: string;
  }

  let {
    open = $bindable(false),
    link = "",
    title = "Proxy",
  }: Props = $props();

  const svg = $derived.by(() => {
    if (!open || !link) return "";
    try {
      return renderSVG(link, {
        whiteColor: "#dee5e0",
        blackColor: "#222a24",
        border: 2,
      });
    } catch {
      return "";
    }
  });

  async function copyLink() {
    try {
      await writeText(link);
      toast.success("Link copied.");
    } catch (e) {
      toast.error(errorMessage(e));
    }
  }
</script>

<Dialog.Root bind:open>
  <Dialog.Content class="sm:max-w-sm">
    <Dialog.Header>
      <Dialog.Title>QR Code</Dialog.Title>
      <Dialog.Description class="truncate" {title}>{title}</Dialog.Description>
    </Dialog.Header>
    <div class="flex flex-col items-center gap-4 py-2">
      {#if svg}
        <div
          class="size-60 overflow-hidden rounded-2xl border bg-olive-200 p-2 [&_svg]:size-full"
        >
          {@html svg}
        </div>
      {:else if open && link}
        <p class="text-destructive text-sm">Failed to render QR.</p>
      {:else}
        <div class="bg-muted size-60 animate-pulse rounded-md"></div>
      {/if}
    </div>
    <Dialog.Footer>
      <Button variant="outline" onclick={copyLink} disabled={!link}>Copy</Button>
      <Button onclick={() => (open = false)}>Close</Button>
    </Dialog.Footer>
  </Dialog.Content>
</Dialog.Root>
