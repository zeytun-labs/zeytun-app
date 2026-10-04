<script lang="ts">
  import * as Popover from "$lib/components/ui/popover/index.js";
  import { HugeiconsIcon } from "@hugeicons/svelte";
  import { cn } from "$lib/utils";
  import {
    PROFILE_ICON_NAMES,
    PROFILE_ICON_MAP,
    type ProfileIconName,
  } from "$lib/core/profile-icons";
  import Button from "../ui/button/button.svelte";

  interface Props {
    value: ProfileIconName | null | undefined;
    onSelect: (icon: ProfileIconName | null) => void;
    fallbackLabel?: string;
  }

  let { value, onSelect, fallbackLabel = "?" }: Props = $props();
  let open = $state(false);
</script>

<Popover.Root bind:open>
  <Popover.Trigger>
    {#snippet child({ props })}
      <Button {...props} size="icon" variant="outline" class="font-light">
        {#key value}
          {#if value && PROFILE_ICON_MAP[value]}
            <HugeiconsIcon icon={PROFILE_ICON_MAP[value]} />
          {:else}
            {fallbackLabel.charAt(0).toUpperCase()}
          {/if}
        {/key}
      </Button>
    {/snippet}
  </Popover.Trigger>
  <Popover.Content class="p-2 grid grid-cols-7 gap-1" align="start">
    <Button
      title="No icon"
      size="icon"
      variant="ghost"
      class={cn(
        "font-light",
        !value &&
          "border-primary bg-primary/10 text-primary hover:bg-primary/10 hover:text-primary",
      )}
      onclick={() => {
        onSelect(null);
        open = false;
      }}
    >
      {fallbackLabel.charAt(0).toUpperCase()}
    </Button>
    {#each PROFILE_ICON_NAMES as name (name)}
      <Button
        type="button"
        title={name.replace(/Icon$/, "")}
        size="icon"
        variant="ghost"
        class={cn(
          value === name &&
            "border-primary bg-primary/10 text-primary hover:bg-primary/10 hover:text-primary",
        )}
        onclick={() => {
          onSelect(name);
          open = false;
        }}
      >
        <HugeiconsIcon icon={PROFILE_ICON_MAP[name]} class="size-4" />
      </Button>
    {/each}
  </Popover.Content>
</Popover.Root>
