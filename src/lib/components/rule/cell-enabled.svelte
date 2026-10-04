<script lang="ts">
  import { Switch } from "$lib/components/ui/switch/index.js";

  interface Props {
    checked: boolean;
    disabled?: boolean;
    onChange: (checked: boolean) => void;
  }

  let { checked, disabled = false, onChange }: Props = $props();
</script>

<!-- stopPropagation so row select/edit doesn't fire -->
<div
  class="flex items-center min-w-6 max-w-6"
  onclick={(e) => e.stopPropagation()}
  onkeydown={(e) => e.stopPropagation()}
  onpointerdown={(e) => e.stopPropagation()}
  onpointerup={(e) => e.stopPropagation()}
  role="presentation"
>
  <Switch
    size="sm"
    {checked}
    {disabled}
    onpointerdown={(e) => e.stopPropagation()}
    onpointerup={(e) => e.stopPropagation()}
    onCheckedChange={(v) => {
      if (typeof v === "boolean") onChange(v);
    }}
  />
</div>
