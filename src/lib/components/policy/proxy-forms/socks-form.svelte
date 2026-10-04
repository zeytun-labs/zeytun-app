<script lang="ts">
  import * as Form from "$lib/components/ui/form/index.js";
  import { Input } from "$lib/components/ui/input/index.js";
  import * as Select from "$lib/components/ui/select/index.js";
  import type { SuperForm } from "sveltekit-superforms";
  import type { ProxyFormSchema } from "$lib/core/proxy-schema.js";
  import { getContext } from "svelte";

  const form = getContext<SuperForm<ProxyFormSchema, any>>("proxyForm");
  const { form: formData } = form;

  let versionStr = $derived(
    $formData.protocolFields?.version != null
      ? String($formData.protocolFields.version)
      : "5",
  );

  function handleVersionChange(val: string) {
    form.form.update((data) => ({
      ...data,
      protocolFields: {
        ...data.protocolFields,
        version: Number(val),
      },
    }));
  }
</script>

<div class="grid gap-3">
  <Form.Field {form} name="protocolFields.version">
    <Form.Control>
      {#snippet children({ props })}
        <Form.Label>Version</Form.Label>
        <Select.Root
          type="single"
          value={versionStr}
          onValueChange={handleVersionChange}
          name={props.name}
        >
          <Select.Trigger {...props}>
            SOCKS{versionStr}
          </Select.Trigger>
          <Select.Content>
            <Select.Group>
              <Select.Item value="4" label="SOCKS4">SOCKS4</Select.Item>
              <Select.Item value="5" label="SOCKS5">SOCKS5</Select.Item>
            </Select.Group>
          </Select.Content>
        </Select.Root>
      {/snippet}
    </Form.Control>
    <Form.FieldErrors />
  </Form.Field>
  <div class="grid grid-cols-2 gap-3">
    <Form.Field {form} name="protocolFields.username">
      <Form.Control>
        {#snippet children({ props })}
          <Form.Label>
            Username
            <span class="text-muted-foreground text-xs font-normal"
              >(optional)</span
            >
          </Form.Label>
          <Input
            {...props}
            bind:value={$formData.protocolFields.username}
            placeholder="Username"
          />
        {/snippet}
      </Form.Control>
      <Form.FieldErrors />
    </Form.Field>

    <Form.Field {form} name="protocolFields.password">
      <Form.Control>
        {#snippet children({ props })}
          <Form.Label>
            Password
            <span class="text-muted-foreground text-xs font-normal"
              >(optional)</span
            >
          </Form.Label>
          <Input
            {...props}
            bind:value={$formData.protocolFields.password}
            placeholder="Password"
          />
        {/snippet}
      </Form.Control>
      <Form.FieldErrors />
    </Form.Field>
  </div>
</div>
