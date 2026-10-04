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

  <div class="grid grid-cols-2 gap-3">
    <Form.Field {form} name="protocolFields.congestion_control">
      <Form.Control>
        {#snippet children({ props })}
          <Form.Label>Congestion Control</Form.Label>
          <Select.Root
            type="single"
            bind:value={$formData.protocolFields.congestion_control}
            name={props.name}
          >
            <Select.Trigger class="w-full" {...props}>
              {$formData.protocolFields.congestion_control ||
                "Select..."}
            </Select.Trigger>
            <Select.Content>
              <Select.Item value="cubic" label="Cubic">Cubic</Select.Item>
              <Select.Item value="bbr" label="BBR">BBR</Select.Item>
              <Select.Item value="new_reno" label="New Reno"
                >New Reno</Select.Item
              >
            </Select.Content>
          </Select.Root>
        {/snippet}
      </Form.Control>
      <Form.FieldErrors />
    </Form.Field>

    <Form.Field {form} name="protocolFields.udp_relay_mode">
      <Form.Control>
        {#snippet children({ props })}
          <Form.Label>UDP Relay Mode</Form.Label>
          <Select.Root
            type="single"
            bind:value={$formData.protocolFields.udp_relay_mode}
            name={props.name}
          >
            <Select.Trigger class="w-full" {...props}>
              {$formData.protocolFields.udp_relay_mode || "Select..."}
            </Select.Trigger>
            <Select.Content>
              <Select.Item value="native" label="Native">Native</Select.Item>
              <Select.Item value="quic" label="QUIC">QUIC</Select.Item>
            </Select.Content>
          </Select.Root>
        {/snippet}
      </Form.Control>
      <Form.FieldErrors />
    </Form.Field>
  </div>

  <Form.Field
    {form}
    name="protocolFields.udp_over_stream"
    class="flex items-center justify-between"
  >
    <Form.Control>
      {#snippet children({ props })}
        <Form.Label>UDP over Stream</Form.Label>
        <Switch
          {...props}
          bind:checked={$formData.protocolFields.udp_over_stream}
        />
      {/snippet}
    </Form.Control>
  </Form.Field>

  <Form.Field
    {form}
    name="protocolFields.zero_rtt_handshake"
    class="flex items-center justify-between"
  >
    <Form.Control>
      {#snippet children({ props })}
        <Form.Label>0-RTT Handshake</Form.Label>
        <Switch
          {...props}
          bind:checked={$formData.protocolFields.zero_rtt_handshake}
        />
      {/snippet}
    </Form.Control>
  </Form.Field>

  <Form.Field {form} name="protocolFields.heartbeat">
    <Form.Control>
      {#snippet children({ props })}
        <Form.Label>
          Heartbeat
          <span class="text-muted-foreground text-xs font-normal"
            >(optional)</span
          >
        </Form.Label>
        <Input
          {...props}
          bind:value={$formData.protocolFields.heartbeat}
          placeholder="10s"
        />
      {/snippet}
    </Form.Control>
    <Form.FieldErrors />
  </Form.Field>
</div>
