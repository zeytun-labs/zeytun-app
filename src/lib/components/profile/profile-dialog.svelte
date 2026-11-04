<script lang="ts">
  import * as Dialog from "$lib/components/ui/dialog/index.js";
  import { Button } from "$lib/components/ui/button/index.js";
  import { Input } from "$lib/components/ui/input/index.js";
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
    name: string;
    icon: ProfileIconName | null | undefined;
  }

  interface Props {
    open: boolean;
    mode: "create" | "edit";
    initialName?: string;
    initialIcon?: string | null;
    saving: boolean;
    onSave: (values: ProfileDialogValues) => void;
  }

  let {
    open = $bindable(false),
    mode,
    initialName = "",
    initialIcon = null,
    saving,
    onSave,
  }: Props = $props();

  const form = superForm<ProfileFormSchema>(
    {
      name: "",
      icon: null,
    },
    {
      SPA: true,
      validators: zod4(profileFormSchema as any),
      onUpdate({ form: f }) {
        if (f.valid) {
          onSave({
            name: f.data.name,
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
          name: initialName,
          icon: (initialIcon as ProfileIconName | null) ?? null,
        },
      });
    }
  });
</script>

<Dialog.Root bind:open>
  <Dialog.Content class="sm:max-w-md">
    <Dialog.Header>
      <Dialog.Title>
        {mode === "create" ? "Add new Profile" : "Edit Profile"}
      </Dialog.Title>
      <Dialog.Description>
        A profile owns its own proxies, policies, rules, and subscription sources.
      </Dialog.Description>
    </Dialog.Header>

    <form use:enhance>
      <div class="grid gap-4 py-4">
        <Form.Field {form} name="name">
          <Form.Control>
            {#snippet children({ props })}
              <Form.Label>Name</Form.Label>
              <div class="flex gap-2">
                <ProfileIconPicker
                  value={$formData.icon}
                  fallbackLabel={$formData.name || "P"}
                  onSelect={(icon) => ($formData.icon = icon)}
                />
                <Input
                  {...props}
                  bind:value={$formData.name}
                  placeholder="Home"
                  required
                  maxlength={100}
                />
              </div>
            {/snippet}
          </Form.Control>
          <Form.FieldErrors />
        </Form.Field>
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
