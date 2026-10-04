<script lang="ts">
  import * as Form from "$lib/components/ui/form/index.js";
  import { Input } from "$lib/components/ui/input/index.js";
  import { Switch } from "$lib/components/ui/switch/index.js";
  import * as Select from "$lib/components/ui/select/index.js";
  import type { SuperForm } from "sveltekit-superforms";
  import type { ProxyFormSchema } from "$lib/core/proxy-schema.js";
  import { getContext } from "svelte";

  const form = getContext<SuperForm<ProxyFormSchema, any>>("proxyForm");
  const { form: formData } = form;
</script>

<div class="grid gap-3">
  <Form.Field {form} name="protocolFields.password">
    <Form.Control>
      {#snippet children({ props })}
        <Form.Label>Password</Form.Label>
        <Input
          {...props}
          bind:value={$formData.protocolFields.password}
          placeholder="Password"
        />
      {/snippet}
    </Form.Control>
    <Form.FieldErrors />
  </Form.Field>

  <div class="flex gap-3">
    <Form.Field {form} class="flex-1" name="protocolFields.encryption">
      <Form.Control>
        {#snippet children({ props })}
          <Form.Label>Encryption</Form.Label>
          <Select.Root
            type="single"
            bind:value={$formData.protocolFields.encryption}
            name={props.name}
          >
            <Select.Trigger class="w-full" {...props}>
              {$formData.protocolFields.encryption ||
                "Select encryption..."}
            </Select.Trigger>
            <Select.Content>
              <Select.Item value="aes-128-gcm" label="AES-128-GCM"
                >AES-128-GCM</Select.Item
              >
              <Select.Item value="aes-256-gcm" label="AES-256-GCM"
                >AES-256-GCM</Select.Item
              >
              <Select.Item
                value="chacha20-ietf-poly1305"
                label="ChaCha20-IETF-Poly1305"
                >ChaCha20-IETF-Poly1305</Select.Item
              >
              <Select.Item
                value="2022-blake3-aes-128-gcm"
                label="2022-Blake3-AES-128-GCM"
                >2022-Blake3-AES-128-GCM</Select.Item
              >
              <Select.Item
                value="2022-blake3-aes-256-gcm"
                label="2022-Blake3-AES-256-GCM"
                >2022-Blake3-AES-256-GCM</Select.Item
              >
              <Select.Item
                value="2022-blake3-chacha20-poly1305"
                label="2022-Blake3-ChaCha20-Poly1305"
                >2022-Blake3-ChaCha20-Poly1305</Select.Item
              >
              <Select.Item value="none" label="None">None</Select.Item>
              <Select.Item value="plain" label="Plain">Plain</Select.Item>
            </Select.Content>
          </Select.Root>
        {/snippet}
      </Form.Control>
      <Form.FieldErrors />
    </Form.Field>

    <Form.Field {form} class="flex-1" name="protocolFields.plugin">
      <Form.Control>
        {#snippet children({ props })}
          <Form.Label>
            Plugin
            <span class="text-muted-foreground text-xs font-normal"
              >(optional)</span
            >
          </Form.Label>
          <Select.Root
            type="single"
            bind:value={$formData.protocolFields.plugin}
            name={props.name}
          >
            <Select.Trigger class="w-full" {...props}>
              {$formData.protocolFields.plugin || "None"}
            </Select.Trigger>
            <Select.Content>
              <Select.Item value="" label="None">None</Select.Item>
              <Select.Item value="obfs-local" label="obfs-local"
                >obfs-local</Select.Item
              >
              <Select.Item value="v2ray-plugin" label="v2ray-plugin"
                >v2ray-plugin</Select.Item
              >
            </Select.Content>
          </Select.Root>
        {/snippet}
      </Form.Control>
      <Form.FieldErrors />
    </Form.Field>
  </div>

  {#if $formData.protocolFields.plugin}
    <Form.Field {form} name="protocolFields.plugin_args">
      <Form.Control>
        {#snippet children({ props })}
          <Form.Label>
            Plugin Arguments
            <span class="text-muted-foreground text-xs font-normal"
              >(optional)</span
            >
          </Form.Label>
          <Input
            {...props}
            bind:value={$formData.protocolFields.plugin_args}
            placeholder="obfs=http;obfs-host=example.com"
          />
        {/snippet}
      </Form.Control>
      <Form.FieldErrors />
    </Form.Field>
  {/if}

  <Form.Field
    {form}
    name="protocolFields.udp_over_tcp"
    class="flex flex-row items-center justify-between"
  >
    <Form.Control>
      {#snippet children({ props })}
        <Form.Label>UDP over TCP</Form.Label>
        <Switch
          {...props}
          bind:checked={$formData.protocolFields.udp_over_tcp}
        />
      {/snippet}
    </Form.Control>
  </Form.Field>
</div>
