<script lang="ts">
  import * as Form from "$lib/components/ui/form/index.js";
  import { Input } from "$lib/components/ui/input/index.js";
  import { Switch } from "$lib/components/ui/switch/index.js";
  import * as Select from "$lib/components/ui/select/index.js";
  import { Textarea } from "$lib/components/ui/textarea/index.js";
  import { Separator } from "$lib/components/ui/separator/index.js";
  import type { SuperForm } from "sveltekit-superforms";
  import type { ProxyFormSchema } from "$lib/core/proxy-schema.js";
  import { getContext } from "svelte";

  const form = getContext<SuperForm<ProxyFormSchema, any>>("proxyForm");
  const { form: formData } = form;

  const tlsVersions = ["1.0", "1.1", "1.2", "1.3"];
</script>

<div class="flex flex-col gap-3">
  {#if $formData.settings.advanced?.enabled}
    <!-- Dial Fields Section -->
    <div class="grid grid-cols-[min-content_1fr] items-center gap-4 pb-1">
      <p class="text-xs uppercase text-muted-foreground whitespace-nowrap">
        dial fields
      </p>
      <Separator class="w-auto!"></Separator>
    </div>

    <div class="grid grid-cols-2 gap-3">
      <Form.Field
        {form}
        name="settings.advanced.reuse_address"
        class="flex flex-row items-center justify-between"
      >
        <Form.Control>
          {#snippet children({ props })}
            <Form.Label>Reuse Address</Form.Label>
            <Switch
              name={props.name}
              checked={!!$formData.settings.advanced.reuse_address}
              onCheckedChange={(checked) => {
                $formData.settings.advanced.reuse_address = checked;
              }}
            />
          {/snippet}
        </Form.Control>
      </Form.Field>

      <Form.Field
        {form}
        name="settings.advanced.tcp_fast_open"
        class="flex flex-row items-center justify-between"
      >
        <Form.Control>
          {#snippet children({ props })}
            <Form.Label>TCP Fast Open</Form.Label>
            <Switch
              name={props.name}
              checked={!!$formData.settings.advanced.tcp_fast_open}
              onCheckedChange={(checked) => {
                $formData.settings.advanced.tcp_fast_open = checked;
              }}
            />
          {/snippet}
        </Form.Control>
      </Form.Field>

      <Form.Field
        {form}
        name="settings.advanced.udp_fragment"
        class="flex flex-row items-center justify-between"
      >
        <Form.Control>
          {#snippet children({ props })}
            <Form.Label>UDP Fragment</Form.Label>
            <Switch
              name={props.name}
              checked={!!$formData.settings.advanced.udp_fragment}
              onCheckedChange={(checked) => {
                $formData.settings.advanced.udp_fragment = checked;
              }}
            />
          {/snippet}
        </Form.Control>
      </Form.Field>

      <Form.Field
        {form}
        name="settings.advanced.tcp_multi_path"
        class="flex flex-row items-center justify-between"
      >
        <Form.Control>
          {#snippet children({ props })}
            <Form.Label>TCP MultiPath</Form.Label>
            <Switch
              name={props.name}
              checked={!!$formData.settings.advanced.tcp_multi_path}
              onCheckedChange={(checked) => {
                $formData.settings.advanced.tcp_multi_path = checked;
              }}
            />
          {/snippet}
        </Form.Control>
      </Form.Field>
    </div>

    <Form.Field {form} name="settings.advanced.connect_timeout">
      <Form.Control>
        {#snippet children({ props })}
          <Form.Label>
            Connect Timeout
            <span class="text-muted-foreground text-xs font-normal"
              >(optional)</span
            >
          </Form.Label>
          <Input
            {...props}
            type="number"
            bind:value={$formData.settings.advanced.connect_timeout}
            placeholder="ms"
          />
        {/snippet}
      </Form.Control>
      <Form.FieldErrors />
    </Form.Field>

    <!-- TLS Section -->
    <div class="grid grid-cols-[min-content_1fr] items-center gap-4 pb-1 mt-2">
      <p class="text-xs uppercase text-muted-foreground whitespace-nowrap">
        tls advanced settings
      </p>
      <Separator class="w-auto!"></Separator>
    </div>

    <div class="grid grid-cols-2 gap-3">
      <Form.Field
        {form}
        name="settings.advanced.disable_sni"
        class="flex flex-row items-center justify-between"
      >
        <Form.Control>
          {#snippet children({ props })}
            <Form.Label>Disable SNI</Form.Label>
            <Switch
              name={props.name}
              checked={!!$formData.settings.advanced.disable_sni}
              onCheckedChange={(checked) => {
                $formData.settings.advanced.disable_sni = checked;
              }}
            />
          {/snippet}
        </Form.Control>
      </Form.Field>

      <Form.Field
        {form}
        name="settings.advanced.enable_ech"
        class="flex flex-row items-center justify-between"
      >
        <Form.Control>
          {#snippet children({ props })}
            <Form.Label>Enable ECH</Form.Label>
            <Switch
              name={props.name}
              checked={!!$formData.settings.advanced.enable_ech}
              onCheckedChange={(checked) => {
                $formData.settings.advanced.enable_ech = checked;
              }}
            />
          {/snippet}
        </Form.Control>
      </Form.Field>
    </div>

    <div class="grid grid-cols-2 gap-3">
      <Form.Field {form} name="settings.advanced.tls_min_version">
        <Form.Control>
          {#snippet children({ props })}
            <Form.Label>
              TLS Min Version
              <span class="text-muted-foreground text-xs font-normal"
                >(optional)</span
              >
            </Form.Label>
            <Select.Root
              type="single"
              bind:value={$formData.settings.advanced.tls_min_version}
              name={props.name}
            >
              <Select.Trigger {...props} class="w-full">
                {$formData.settings.advanced.tls_min_version || "Default"}
              </Select.Trigger>
              <Select.Content>
                <Select.Item value="" label="Default">Default</Select.Item>
                {#each tlsVersions as tv}
                  <Select.Item value={tv} label={tv}>{tv}</Select.Item>
                {/each}
              </Select.Content>
            </Select.Root>
          {/snippet}
        </Form.Control>
        <Form.FieldErrors />
      </Form.Field>

      <Form.Field {form} name="settings.advanced.tls_max_version">
        <Form.Control>
          {#snippet children({ props })}
            <Form.Label>
              TLS Max Version
              <span class="text-muted-foreground text-xs font-normal"
                >(optional)</span
              >
            </Form.Label>
            <Select.Root
              type="single"
              bind:value={$formData.settings.advanced.tls_max_version}
              name={props.name}
            >
              <Select.Trigger {...props} class="w-full">
                {$formData.settings.advanced.tls_max_version || "Default"}
              </Select.Trigger>
              <Select.Content>
                <Select.Item value="" label="Default">Default</Select.Item>
                {#each tlsVersions as tv}
                  <Select.Item value={tv} label={tv}>{tv}</Select.Item>
                {/each}
              </Select.Content>
            </Select.Root>
          {/snippet}
        </Form.Control>
        <Form.FieldErrors />
      </Form.Field>
    </div>

    {#if $formData.settings.advanced.enable_ech}
      <Form.Field {form} name="settings.advanced.ech_config">
        <Form.Control>
          {#snippet children({ props })}
            <Form.Label>
              ECH Config
              <span class="text-muted-foreground text-xs font-normal"
                >(optional)</span
              >
            </Form.Label>
            <Textarea
              {...props}
              bind:value={$formData.settings.advanced.ech_config}
              placeholder="Base64 encoded ECH config"
              class="font-mono text-xs"
            />
          {/snippet}
        </Form.Control>
        <Form.FieldErrors />
      </Form.Field>
    {/if}

    <Form.Field {form} name="settings.advanced.certificate_sha256">
      <Form.Control>
        {#snippet children({ props })}
          <Form.Label>
            Certificate SHA256
            <span class="text-muted-foreground text-xs font-normal"
              >(optional)</span
            >
          </Form.Label>
          <Textarea
            {...props}
            bind:value={$formData.settings.advanced.certificate_sha256}
            placeholder="SHA256 hash"
            class="font-mono text-xs"
          />
        {/snippet}
      </Form.Control>
      <Form.FieldErrors />
    </Form.Field>

    <Form.Field {form} name="settings.advanced.client_cert">
      <Form.Control>
        {#snippet children({ props })}
          <Form.Label>
            Client Certificate
            <span class="text-muted-foreground text-xs font-normal"
              >(optional)</span
            >
          </Form.Label>
          <Textarea
            {...props}
            bind:value={$formData.settings.advanced.client_cert}
            placeholder="-----BEGIN CERTIFICATE-----..."
            class="font-mono text-xs h-24"
          />
        {/snippet}
      </Form.Control>
      <Form.FieldErrors />
    </Form.Field>

    <Form.Field {form} name="settings.advanced.client_key">
      <Form.Control>
        {#snippet children({ props })}
          <Form.Label>
            Client Key
            <span class="text-muted-foreground text-xs font-normal"
              >(optional)</span
            >
          </Form.Label>
          <Textarea
            {...props}
            bind:value={$formData.settings.advanced.client_key}
            placeholder="-----BEGIN PRIVATE KEY-----..."
            class="font-mono text-xs h-24"
          />
        {/snippet}
      </Form.Control>
      <Form.FieldErrors />
    </Form.Field>
  {/if}
</div>
