<script lang="ts">
  import * as Form from "$lib/components/ui/form/index.js";
  import { Input } from "$lib/components/ui/input/index.js";
  import { Switch } from "$lib/components/ui/switch/index.js";
  import { Separator } from "$lib/components/ui/separator/index.js";
  import type { SuperForm } from "sveltekit-superforms";
  import type { ProxyFormSchema } from "$lib/core/proxy-schema.js";
  import { getContext } from "svelte";

  const form = getContext<SuperForm<ProxyFormSchema, any>>("proxyForm");
  const { form: formData } = form;
  
</script>

<div class="flex flex-col gap-3">
  {#if $formData.settings.multiplex?.enabled}
    <div class="grid gap-3">
      <Form.Field
        {form}
        name="settings.multiplex.tcp_brutal"
        class="flex flex-row items-center justify-between"
      >
        <Form.Control>
          {#snippet children({ props })}
            <Form.Label>Enable TCP Brutal</Form.Label>
            <Switch
              name={props.name}
              checked={!!$formData.settings.multiplex.tcp_brutal}
              onCheckedChange={(checked) => {
                $formData.settings.multiplex.tcp_brutal = checked;
              }}
            />
          {/snippet}
        </Form.Control>
      </Form.Field>

      <div class="grid grid-cols-1 gap-3">
        <Form.Field {form} name="settings.multiplex.brutal_download_speed">
          <Form.Control>
            {#snippet children({ props })}
              <Form.Label>
                Brutal Download Speed
                <span class="text-muted-foreground text-xs font-normal"
                  >(optional)</span
                >
              </Form.Label>
              <Input
                {...props}
                type="number"
                bind:value={$formData.settings.multiplex.brutal_download_speed}
                placeholder="Mbps"
              />
            {/snippet}
          </Form.Control>
          <Form.FieldErrors />
        </Form.Field>

        <Form.Field {form} name="settings.multiplex.brutal_upload_speed">
          <Form.Control>
            {#snippet children({ props })}
              <Form.Label>
                Brutal Upload Speed
                <span class="text-muted-foreground text-xs font-normal"
                  >(optional)</span
                >
              </Form.Label>
              <Input
                {...props}
                type="number"
                bind:value={$formData.settings.multiplex.brutal_upload_speed}
                placeholder="Mbps"
              />
            {/snippet}
          </Form.Control>
          <Form.FieldErrors />
        </Form.Field>
      </div>
    </div>
  {/if}
</div>
