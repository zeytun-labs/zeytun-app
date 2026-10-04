<script lang="ts">
  import * as Form from "$lib/components/ui/form/index.js";
  import { Input } from "$lib/components/ui/input/index.js";
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

  <div class="grid grid-cols-2 gap-3">
    <Form.Field {form} name="protocolFields.up_mbps">
      <Form.Control>
        {#snippet children({ props })}
          <Form.Label>Up (Mbps)</Form.Label>
          <Input
            {...props}
            type="number"
            bind:value={$formData.protocolFields.up_mbps}
            placeholder="50"
          />
        {/snippet}
      </Form.Control>
      <Form.FieldErrors />
    </Form.Field>

    <Form.Field {form} name="protocolFields.down_mbps">
      <Form.Control>
        {#snippet children({ props })}
          <Form.Label>Down (Mbps)</Form.Label>
          <Input
            {...props}
            type="number"
            bind:value={$formData.protocolFields.down_mbps}
            placeholder="100"
          />
        {/snippet}
      </Form.Control>
      <Form.FieldErrors />
    </Form.Field>
  </div>

  <Form.Field {form} name="protocolFields.obf_password">
    <Form.Control>
      {#snippet children({ props })}
        <Form.Label>
          Obfuscation Password
          <span class="text-muted-foreground text-xs font-normal"
            >(optional)</span
          >
        </Form.Label>
        <Input
          {...props}
          bind:value={$formData.protocolFields.obf_password}
          placeholder="Optional obfuscation password"
        />
      {/snippet}
    </Form.Control>
    <Form.FieldErrors />
  </Form.Field>

  <Form.Field {form} name="protocolFields.server_ports">
    <Form.Control>
      {#snippet children({ props })}
        <Form.Label>
          Server Ports
          <span class="text-muted-foreground text-xs font-normal"
            >(optional)</span
          >
        </Form.Label>
        <Input
          {...props}
          bind:value={$formData.protocolFields.server_ports}
          placeholder="443,8443 or 20000-40000"
        />
      {/snippet}
    </Form.Control>
    <Form.FieldErrors />
  </Form.Field>

  <Form.Field {form} name="protocolFields.hop_interval">
    <Form.Control>
      {#snippet children({ props })}
        <Form.Label>
          Hop Interval
          <span class="text-muted-foreground text-xs font-normal"
            >(optional)</span
          >
        </Form.Label>
        <Input
          {...props}
          bind:value={$formData.protocolFields.hop_interval}
          placeholder="30s"
        />
      {/snippet}
    </Form.Control>
    <Form.FieldErrors />
  </Form.Field>
</div>
