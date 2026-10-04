<script lang="ts">
  import * as Select from "$lib/components/ui/select/index.js";
  import { Input } from "$lib/components/ui/input/index.js";
  import * as InputGroup from "$lib/components/ui/input-group";
  import { HugeiconsIcon } from "@hugeicons/svelte";
  import { Search01Icon } from "@hugeicons/core-free-icons";

  let { sortBy = $bindable("traffic"), searchQuery = $bindable("") } = $props<{
    sortBy: "speed" | "traffic" | "name";
    searchQuery: string;
  }>();

  const selectOptions = [
    { value: "traffic", label: "Sort by Traffic" },
    { value: "speed", label: "Sort by Speed" },
    { value: "name", label: "Sort by Name" },
  ];

  const triggerContent = $derived(
    selectOptions.find((f) => f.value === sortBy)?.label ?? "Select an Option",
  );
</script>

<div class="flex items-center justify-between gap-4">

  <div class="relative w-full max-w-xs">
    <InputGroup.Root variant="backless">
      <InputGroup.Input placeholder="Search..." bind:value={searchQuery} />
      <InputGroup.Addon>
        <HugeiconsIcon icon={Search01Icon} />
      </InputGroup.Addon>
    </InputGroup.Root>
  </div>

  <div class="flex items-center gap-3">
    <Select.Root type="single" name="favoriteFruit" bind:value={sortBy}>
      <Select.Trigger class="w-45" variant="backless">
        {triggerContent}
      </Select.Trigger>
      <Select.Content>
        {#each selectOptions as sortOption (sortOption.value)}
          <Select.Item value={sortOption.value} label={sortOption.label}>
            {sortOption.label}
          </Select.Item>
        {/each}
      </Select.Content>
    </Select.Root>
  </div>
</div>
