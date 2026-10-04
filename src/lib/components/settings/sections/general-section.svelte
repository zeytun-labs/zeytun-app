<script lang="ts">
  import { Switch } from "$lib/components/ui/switch/index.js";
  import { Label } from "$lib/components/ui/label/index.js";
  import * as Select from "$lib/components/ui/select/index.js";
  import * as Tabs from "$lib/components/ui/tabs/index.js";
  import { HugeiconsIcon } from "@hugeicons/svelte";
  import {
    Sun01Icon,
    Moon02Icon,
    ComputerIcon,
  } from "@hugeicons/core-free-icons";
  import { themeOptions, type Theme } from "$lib/theme.svelte";
  import type { SettingsDraft } from "../settings-draft.svelte";
  import SettingRow from "../setting-row.svelte";
  import SettingsSection from "../settings-section.svelte";
  import type { LogLevel } from "../settings-draft.svelte";

  let { draft }: { draft: SettingsDraft } = $props();

  const logLevels: { id: LogLevel; label: string }[] = [
    { id: "trace", label: "Trace" },
    { id: "debug", label: "Debug" },
    { id: "info", label: "Info" },
    { id: "warn", label: "Warn" },
    { id: "error", label: "Error" },
    { id: "fatal", label: "Fatal" },
    { id: "panic", label: "Panic" },
  ];

  function themeIcon(value: Theme) {
    return value === "dark" ? Moon02Icon : value === "light" ? Sun01Icon : ComputerIcon;
  }
</script>

<SettingsSection
  title="General"
  description="App behavior, appearance, and logging."
>
  <SettingRow
    label="Launch at Login"
    description="Automatically start Zeytun when you log in."
  >
    <Switch bind:checked={draft.settings.launchAtLogin} />
  </SettingRow>

  <SettingRow
    label="Hide on Close"
    description="Hide to tray and dock when closing the window instead of quitting."
  >
    <Switch bind:checked={draft.settings.hideOnClose} />
  </SettingRow>

  <SettingRow label="Log Level" for="logLevel">
    <div class="w-32">
      <Select.Root
        type="single"
        value={draft.settings.logLevel}
        onValueChange={(v: string) => (draft.settings.logLevel = v)}
      >
        <Select.Trigger class="h-9 w-full">
          {logLevels.find((l) => l.id === draft.settings.logLevel)?.label ?? "Info"}
        </Select.Trigger>
        <Select.Content>
          <Select.Group>
            {#each logLevels as level (level.id)}
              <Select.Item value={level.id} label={level.label} />
            {/each}
          </Select.Group>
        </Select.Content>
      </Select.Root>
    </div>
  </SettingRow>

  <!-- Theme switch disabled - force dark mode for now
  <div class="flex items-center justify-between gap-6 px-5 py-4">
    <Label>Theme</Label>
    <Tabs.Root
      value={draft.theme}
      onValueChange={(v: string) => (draft.theme = v as Theme)}
    >
      <Tabs.List variant="primary">
        {#each themeOptions as theme (theme.value)}
          <Tabs.Trigger value={theme.value}>
            <HugeiconsIcon class="size-4" icon={themeIcon(theme.value)} />
            {theme.label}
          </Tabs.Trigger>
        {/each}
      </Tabs.List>
    </Tabs.Root>
  </div>
  -->
</SettingsSection>
