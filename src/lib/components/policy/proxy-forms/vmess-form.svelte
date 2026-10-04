<script lang="ts">
  import * as Form from "$lib/components/ui/form/index.js";
  import { Input } from "$lib/components/ui/input/index.js";
  import * as Select from "$lib/components/ui/select/index.js";
  import type { SuperForm } from "sveltekit-superforms";
  import type { ProxyFormSchema } from "$lib/core/proxy-schema.js";
  import { getContext } from "svelte";

  const form = getContext<SuperForm<ProxyFormSchema, any>>("proxyForm");
  const { form: formData } = form;
</script>

<div class="grid gap-3">
  <Form.Field {form} name="protocolFields.uuid">
    <Form.Control>
      {#snippet children({ props })}
        <Form.Label>UUID</Form.Label>
        <Input
          {...props}
          bind:value={$formData.protocolFields.uuid}
          placeholder="xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx"
        />
      {/snippet}
    </Form.Control>
    <Form.FieldErrors />
  </Form.Field>

  <Form.Field {form} name="protocolFields.alter_id">
    <Form.Control>
      {#snippet children({ props })}
        <Form.Label>Alter ID</Form.Label>
        <Input
          {...props}
          type="number"
          bind:value={$formData.protocolFields.alter_id}
          placeholder="0"
        />
      {/snippet}
    </Form.Control>
    <Form.FieldErrors />
  </Form.Field>

  <div class="flex gap-3">
    <Form.Field {form} class="flex-1" name="protocolFields.security">
      <Form.Control>
        {#snippet children({ props })}
          <Form.Label>Security</Form.Label>
          <Select.Root
            type="single"
            bind:value={$formData.protocolFields.security}
            name={props.name}
          >
            <Select.Trigger class="w-full" {...props}>
              {$formData.protocolFields.security ||
                "Select security..."}
            </Select.Trigger>
            <Select.Content>
              <Select.Item value="auto" label="Auto">Auto</Select.Item>
              <Select.Item value="none" label="None">None</Select.Item>
              <Select.Item value="aes-128-gcm" label="AES-128-GCM"
                >AES-128-GCM</Select.Item
              >
              <Select.Item value="chacha20-poly1305" label="ChaCha20-Poly1305"
                >ChaCha20-Poly1305</Select.Item
              >
              <Select.Item value="zero" label="Zero">Zero</Select.Item>
            </Select.Content>
          </Select.Root>
        {/snippet}
      </Form.Control>
      <Form.FieldErrors />
    </Form.Field>

    <Form.Field {form} class="flex-1" name="protocolFields.packet_encoding">
      <Form.Control>
        {#snippet children({ props })}
          <Form.Label>
            Packet Encoding
            <span class="text-muted-foreground text-xs font-normal"
              >(optional)</span
            >
          </Form.Label>
          <Select.Root
            type="single"
            bind:value={$formData.protocolFields.packet_encoding}
            name={props.name}
          >
            <Select.Trigger class="w-full" {...props}>
              {$formData.protocolFields.packet_encoding || "None"}
            </Select.Trigger>
            <Select.Content>
              <Select.Item value="" label="None">None</Select.Item>
              <Select.Item value="xudp" label="XUDP">XUDP</Select.Item>
              <Select.Item value="packetaddr" label="Packet Address"
                >Packet Address</Select.Item
              >
            </Select.Content>
          </Select.Root>
        {/snippet}
      </Form.Control>
      <Form.FieldErrors />
    </Form.Field>
  </div>
</div>
