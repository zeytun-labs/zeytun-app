<script lang="ts">
  import * as Form from "$lib/components/ui/form/index.js";
  import { Input } from "$lib/components/ui/input/index.js";
  import type { SuperForm } from "sveltekit-superforms";
  import type { ProxyFormSchema } from "$lib/core/proxy-schema.js";
  import { getContext } from "svelte";

  const form = getContext<SuperForm<ProxyFormSchema, any>>("proxyForm");
  const { form: formData } = form;
</script>

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
