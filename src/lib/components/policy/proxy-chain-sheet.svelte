<script lang="ts">
  import * as Sheet from "$lib/components/ui/sheet/index.js";
  import { Button } from "$lib/components/ui/button/index.js";
  import { Input } from "$lib/components/ui/input/index.js";
  import { ScrollArea } from "$lib/components/ui/scroll-area/index.js";
  import * as Select from "$lib/components/ui/select/index.js";
  import { Label } from "$lib/components/ui/label/index.js";
  import type { Proxy } from "$lib/core/types";
  import { HugeiconsIcon } from "@hugeicons/svelte";
  import {
    DragDropVerticalIcon,
    Cancel01Icon,
    Add01Icon,
  } from "@hugeicons/core-free-icons";
  import { useSortable } from "@dnd-kit-svelte/svelte/sortable";
  import { DragDropProvider } from "@dnd-kit-svelte/svelte";
  import { move } from "@dnd-kit/helpers";
  import { RestrictToVerticalAxis } from "@dnd-kit/abstract/modifiers";

  interface Props {
    open: boolean;
    mode: "create" | "edit";
    proxy?: Proxy | null;
    allProxies: Proxy[];
    saving: boolean;
    onSave: (data: { name: string; proxies: string[] }) => void;
  }

  let {
    open = $bindable(false),
    mode,
    proxy,
    allProxies,
    saving,
    onSave,
  }: Props = $props();

  let name = $state("");
  let selectedProxies = $state<string[]>([]);
  let newProxyToAdd = $state<string>("");

  $effect(() => {
    if (open) {
      if (mode === "edit" && proxy && proxy.config?.proxies) {
        name = proxy.title;
        selectedProxies = [...proxy.config.proxies];
      } else {
        name = "";
        selectedProxies = [];
      }
      newProxyToAdd = "";
    }
  });

  const availableProxies = $derived(
    allProxies.filter(
      (p) => p.protocol !== "chain" && !selectedProxies.includes(p.tag),
    ),
  );

  function addProxy() {
    if (newProxyToAdd && !selectedProxies.includes(newProxyToAdd)) {
      selectedProxies = [...selectedProxies, newProxyToAdd];
      newProxyToAdd = "";
    }
  }

  function removeProxy(index: number) {
    selectedProxies = selectedProxies.filter((_, i) => i !== index);
  }

  function handleSubmit(e: Event) {
    e.preventDefault();
    if (!name.trim()) return;
    if (selectedProxies.length < 2) return;
    onSave({ name: name.trim(), proxies: selectedProxies });
  }

  function getProxyName(tag: string) {
    return allProxies.find((p) => p.tag === tag)?.title || tag;
  }
</script>

<Sheet.Root bind:open>
  <Sheet.Content class="flex w-full flex-col sm:max-w-md" side="right">
    <form
      id="proxy-chain-form"
      class="flex flex-1 flex-col overflow-hidden"
      onsubmit={handleSubmit}
    >
      <ScrollArea class="flex-1 px-4 py-4">
        <Sheet.Header>
          <Sheet.Title
            >{mode === "create"
              ? "Create Proxy Chain"
              : "Edit Proxy Chain"}</Sheet.Title
          >
          <Sheet.Description>
            Route traffic through a sequence of proxies. (e.g., Proxy A ➔ Proxy
            B). Traffic will enter the first proxy and exit from the last.
          </Sheet.Description>
        </Sheet.Header>

        <div class="space-y-6 pb-20">
          <div class="space-y-2">
            <Label for="chain-name">Chain Name</Label>
            <Input
              id="chain-name"
              bind:value={name}
              placeholder="e.g., Double VPN"
              required
            />
          </div>

          <div class="space-y-2">
            <Label>Sequence (First to Last)</Label>

            <div class="flex flex-col gap-2">
              <DragDropProvider
                modifiers={[
                  // @ts-ignore
                  RestrictToVerticalAxis,
                ]}
                onDragEnd={(e: any) => {
                  selectedProxies = move(selectedProxies, e);
                }}
              >
                {#each selectedProxies as pTag, i (pTag)}
                  {@render DraggableItem({ pTag, i })}
                {/each}
              </DragDropProvider>

              {#if selectedProxies.length === 0}
                <div
                  class="text-sm text-muted-foreground p-4 text-center border border-dashed rounded-xl"
                >
                  No proxies added yet. Select below to add.
                </div>
              {/if}
            </div>
          </div>

          <div class="space-y-2 pt-4 border-t">
            <Label>Add Proxy to Chain</Label>
            <div class="flex gap-2">
              <Select.Root
                type="single"
                name="new-proxy"
                bind:value={newProxyToAdd}
              >
                <Select.Trigger class="flex-1">
                  {newProxyToAdd
                    ? getProxyName(newProxyToAdd)
                    : "Select a proxy..."}
                </Select.Trigger>
                <Select.Content>
                  <Select.Group>
                    {#each availableProxies as p}
                      <Select.Item value={p.tag} label={p.title}>
                        {p.title}
                      </Select.Item>
                    {/each}
                  </Select.Group>
                </Select.Content>
              </Select.Root>
              <Button
                type="button"
                variant="secondary"
                onclick={addProxy}
                disabled={!newProxyToAdd}
              >
                <HugeiconsIcon icon={Add01Icon} /> Add
              </Button>
            </div>
          </div>
        </div>
      </ScrollArea>

      <div
        class="bg-background mt-auto flex items-center justify-end gap-3 border-t p-4"
      >
        <Button type="button" variant="outline" onclick={() => (open = false)}>
          Cancel
        </Button>
        <Button
          type="submit"
          class="flex-1"
          disabled={saving || !name.trim() || selectedProxies.length < 2}
        >
          {saving ? "Saving..." : "Save"}
        </Button>
      </div>
    </form>
  </Sheet.Content>
</Sheet.Root>

{#snippet DraggableItem({ pTag, i }: { pTag: string; i: number })}
  {@const { ref, isDragging, handleRef } = useSortable({
    id: pTag,
    index: () => i,
  })}
  <div
    class="flex items-center gap-2 bg-muted p-2 rounded-xl data-[dnd-dragging=true]:p-2! data-[dnd-dragging=true]:bg-muted! data-[dnd-dragging=true]:opacity-100 data-[dnd-dragging=true]:z-10 relative"
    data-dragging={isDragging.current}
    {@attach ref}
  >
    <button
      type="button"
      class="p-1 cursor-grab active:cursor-grabbing text-muted-foreground hover:text-foreground"
      {@attach handleRef}
    >
      <HugeiconsIcon icon={DragDropVerticalIcon} class="size-4" />
    </button>
    <div class="flex-1 flex items-center gap-2 px-1">
      <span class="text-sm">{getProxyName(pTag)}</span>
    </div>
    <Button
      type="button"
      variant="ghost"
      size="icon"
      class="h-8 w-8 text-destructive hover:text-destructive hover:bg-destructive/10"
      onclick={() => removeProxy(i)}
    >
      <HugeiconsIcon icon={Cancel01Icon} class="size-4" />
    </Button>
  </div>
{/snippet}
