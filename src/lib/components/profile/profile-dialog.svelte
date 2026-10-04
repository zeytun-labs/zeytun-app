<script lang="ts">
  import * as Dialog from "$lib/components/ui/dialog/index.js";
  import { Button } from "$lib/components/ui/button/index.js";
  import { Input } from "$lib/components/ui/input/index.js";
  import { Checkbox } from "$lib/components/ui/checkbox/index.js";
  import { Label } from "$lib/components/ui/label/index.js";
  import * as Form from "$lib/components/ui/form/index.js";
  import { superForm } from "sveltekit-superforms";
  import { zod4 } from "sveltekit-superforms/adapters";
  import {
    profileFormSchema,
    type ProfileFormSchema,
  } from "$lib/core/profile-schema.js";
  import type { ProfileIconName } from "$lib/core/profile-icons";
  import ProfileIconPicker from "./profile-icon-picker.svelte";

  export interface ProfileDialogValues {
    name: string | undefined;
    url: string | undefined;
    skipAutoUpdate: boolean;
    updateIntervalHours: number;
    icon: ProfileIconName | null | undefined;
  }

  interface Props {
    open: boolean;
    mode: "create" | "edit";
    initialName?: string;
    initialUrl?: string;
    initialSkipAutoUpdate?: boolean;
    initialUpdateIntervalHours?: number;
    initialIcon?: string | null;
    saving: boolean;
    onSave: (values: ProfileDialogValues) => void;
  }

  let {
    open = $bindable(false),
    mode,
    initialName = "",
    initialUrl = "",
    initialSkipAutoUpdate = false,
    initialUpdateIntervalHours = 12,
    initialIcon = null,
    saving,
    onSave,
  }: Props = $props();

  const form = superForm<ProfileFormSchema>(
    {
      name: undefined,
      url: undefined,
      skipAutoUpdate: false,
      updateIntervalHours: 12,
      icon: null,
    },
    {
      SPA: true,
      validators: zod4(profileFormSchema as any),
      onUpdate({ form: f }) {
        if (f.valid) {
          onSave({
            name: f.data.name ?? undefined,
            url: f.data.url ?? undefined,
            skipAutoUpdate: f.data.skipAutoUpdate,
            updateIntervalHours: f.data.updateIntervalHours,
            icon: f.data.icon ?? null,
          });
        }
      },
    },
  );

  const { form: formData, enhance, reset } = form;

  $effect(() => {
    if (open) {
      reset({
        data: {
          name: initialName || undefined,
          url: initialUrl || undefined,
          skipAutoUpdate: initialSkipAutoUpdate,
          updateIntervalHours: initialUpdateIntervalHours,
          icon: (initialIcon as ProfileIconName | null) ?? null,
        },
      });
    }
  });
</script>

<Dialog.Root bind:open>
  <Dialog.Content class="sm:max-w-lg">
    <Dialog.Header>
      <Dialog.Title>
        {mode === "create" ? "Add new Profile" : "Edit Profile"}
      </Dialog.Title>
      <Dialog.Description>
        A profile owns its own proxies, policies, and rules. Optionally provide
        a subscription URL to auto-import proxies.
      </Dialog.Description>
    </Dialog.Header>

    <form use:enhance>
      <div class="grid gap-4 py-4">
        <div class="flex items-end gap-3">
          <Form.Field {form} name="name" class="flex-1">
            <Form.Control>
              {#snippet children({ props })}
                <Form.Label>
                  Name
                  <span class="text-muted-foreground text-xs font-normal"
                    >(optional)</span
                  >
                </Form.Label>
                <div class="flex gap-1">
                  <ProfileIconPicker
                    value={$formData.icon}
                    fallbackLabel={$formData.name || "P"}
                    onSelect={(icon) => ($formData.icon = icon)}
                  />
                  <Input
                    {...props}
                    bind:value={$formData.name}
                    placeholder="Home"
                  />
                </div>
              {/snippet}
            </Form.Control>
            <Form.FieldErrors />
          </Form.Field>
        </div>

        <Form.Field {form} name="url">
          <Form.Control>
            {#snippet children({ props })}
              <Form.Label>
                Subscription URL
                <span class="text-muted-foreground text-xs font-normal"
                  >(optional)</span
                >
              </Form.Label>
              <Input
                {...props}
                bind:value={$formData.url}
                placeholder="https://example.com/subscribe/..."
              />
            {/snippet}
          </Form.Control>
          <Form.FieldErrors />
        </Form.Field>

        <Form.Field {form} name="updateIntervalHours">
          <Form.Control>
            {#snippet children({ props })}
              <Form.Label>Auto-update interval (hours)</Form.Label>
              <Input
                {...props}
                type="number"
                min="1"
                max="720"
                bind:value={$formData.updateIntervalHours}
              />
            {/snippet}
          </Form.Control>
          <Form.FieldErrors />
        </Form.Field>

        <div class="flex items-center gap-2">
          <Checkbox
            id="skip-auto-update"
            bind:checked={$formData.skipAutoUpdate}
          />
          <Label for="skip-auto-update" class="text-sm font-normal">
            Skip automatic update
          </Label>
        </div>
      </div>

      <Dialog.Footer>
        <Button type="button" variant="outline" onclick={() => (open = false)}>
          Cancel
        </Button>
        <Form.Button disabled={saving}>
          {saving ? "Saving..." : "Save"}
        </Form.Button>
      </Dialog.Footer>
    </form>
  </Dialog.Content>
</Dialog.Root>
