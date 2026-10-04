<script lang="ts">
    import * as Form from "$lib/components/ui/form/index.js";
    import { Input } from "$lib/components/ui/input/index.js";
    import { Switch } from "$lib/components/ui/switch/index.js";
    import * as Select from "$lib/components/ui/select/index.js";
    import { Separator } from "$lib/components/ui/separator/index.js";
    import type { SuperForm } from "sveltekit-superforms";
    import type { ProxyFormSchema } from "$lib/core/proxy-schema.js";
    import { getContext } from "svelte";

    const form = getContext<SuperForm<ProxyFormSchema, any>>("proxyForm");
    const { form: formData } = form;

    const fingerprintOptions = [
        "chrome",
        "firefox",
        "edge",
        "ios",
        "android",
        "random",
        "randomized",
        "qq",
        "360",
    ];
</script>

<div class="flex flex-col gap-4">
    {#if $formData.settings.tls?.enabled}
        <div class="grid grid-cols-[min-content_1fr] items-center gap-4 pb-1">
            <p
                class="text-xs uppercase text-muted-foreground whitespace-nowrap"
            >
                tls security settings
            </p>
            <Separator class="w-auto!"></Separator>
        </div>

        <div class="grid gap-3">
            <Form.Field
                {form}
                name="settings.tls.allow_insecure"
                class="flex flex-row items-center justify-between"
            >
                <Form.Control>
                    {#snippet children({ props })}
                        <Form.Label>Allow Insecure</Form.Label>
                        <Switch
                            name={props.name}
                            checked={!!$formData.settings.tls
                                .allow_insecure}
                            onCheckedChange={(checked) => {
                                $formData.settings.tls.allow_insecure =
                                    checked;
                            }}
                        />
                    {/snippet}
                </Form.Control>
            </Form.Field>

            <div class="grid grid-cols-2 gap-3">
                <Form.Field {form} name="settings.tls.sni">
                    <Form.Control>
                        {#snippet children({ props })}
                            <Form.Label>
                                SNI
                                <span
                                    class="text-muted-foreground text-xs font-normal"
                                    >(optional)</span
                                >
                            </Form.Label>
                            <Input
                                {...props}
                                bind:value={$formData.settings.tls.sni}
                                placeholder="example.com"
                            />
                        {/snippet}
                    </Form.Control>
                    <Form.FieldErrors />
                </Form.Field>

                <Form.Field {form} name="settings.tls.alpn">
                    <Form.Control>
                        {#snippet children({ props })}
                            <Form.Label>
                                ALPN
                                <span
                                    class="text-muted-foreground text-xs font-normal"
                                    >(optional)</span
                                >
                            </Form.Label>
                            <Input
                                {...props}
                                bind:value={
                                    $formData.settings.tls.alpn
                                }
                                placeholder="h2,http/1.1"
                            />
                        {/snippet}
                    </Form.Control>
                    <Form.FieldErrors />
                </Form.Field>
            </div>

            <div
                class="grid grid-cols-[min-content_1fr] items-center gap-4 pb-1"
            >
                <p
                    class="text-xs uppercase text-muted-foreground whitespace-nowrap"
                >
                    tls camouflage settings
                </p>
                <Separator class="w-auto!"></Separator>
            </div>

            <div class="grid grid-cols-2 gap-3">
                <Form.Field
                    {form}
                    name="settings.tls.fragment"
                    class="flex flex-row items-center justify-between"
                >
                    <Form.Control>
                        {#snippet children({ props })}
                            <Form.Label>Fragment</Form.Label>
                            <Switch
                                name={props.name}
                                checked={!!$formData.settings.tls
                                    .fragment}
                                onCheckedChange={(checked) => {
                                    $formData.settings.tls.fragment =
                                        checked;
                                }}
                            />
                        {/snippet}
                    </Form.Control>
                </Form.Field>

                <Form.Field
                    {form}
                    name="settings.tls.record_fragment"
                    class="flex flex-row items-center justify-between"
                >
                    <Form.Control>
                        {#snippet children({ props })}
                            <Form.Label>Record Fragment</Form.Label>
                            <Switch
                                name={props.name}
                                checked={!!$formData.settings.tls
                                    .record_fragment}
                                onCheckedChange={(checked) => {
                                    $formData.settings.tls.record_fragment = checked;
                                }}
                            />
                        {/snippet}
                    </Form.Control>
                </Form.Field>
            </div>

            <Form.Field {form} name="settings.tls.fallback_delay">
                <Form.Control>
                    {#snippet children({ props })}
                        <Form.Label>
                            Fallback Delay
                            <span
                                class="text-muted-foreground text-xs font-normal"
                                >(optional)</span
                            >
                        </Form.Label>
                        <Input
                            {...props}
                            bind:value={
                                $formData.settings.tls.fallback_delay
                            }
                            placeholder="300ms"
                        />
                    {/snippet}
                </Form.Control>
                <Form.FieldErrors />
            </Form.Field>

            <Form.Field {form} name="settings.tls.fingerprint">
                <Form.Control>
                    {#snippet children({ props })}
                        <Form.Label>
                            Fingerprint
                            <span
                                class="text-muted-foreground text-xs font-normal"
                                >(optional)</span
                            >
                        </Form.Label>
                        <Select.Root
                            type="single"
                            bind:value={
                                $formData.settings.tls.fingerprint
                            }
                            name={props.name}
                        >
                            <Select.Trigger {...props} class="w-full">
                                {$formData.settings.tls.fingerprint ||
                                    "Select fingerprint..."}
                            </Select.Trigger>
                            <Select.Content>
                                {#each fingerprintOptions as fp}
                                    <Select.Item value={fp} label={fp}
                                        >{fp}</Select.Item
                                    >
                                {/each}
                            </Select.Content>
                        </Select.Root>
                    {/snippet}
                </Form.Control>
                <Form.FieldErrors />
            </Form.Field>

            <div class="grid grid-cols-2 gap-3">
                <Form.Field {form} name="settings.tls.reality_pbk">
                    <Form.Control>
                        {#snippet children({ props })}
                            <Form.Label>
                                Reality Public Key
                                <span
                                    class="text-muted-foreground text-xs font-normal"
                                    >(optional)</span
                                >
                            </Form.Label>
                            <Input
                                {...props}
                                bind:value={
                                    $formData.settings.tls.reality_pbk
                                }
                                placeholder="Public key"
                            />
                        {/snippet}
                    </Form.Control>
                    <Form.FieldErrors />
                </Form.Field>

                <Form.Field {form} name="settings.tls.reality_sid">
                    <Form.Control>
                        {#snippet children({ props })}
                            <Form.Label>
                                Reality Short ID
                                <span
                                    class="text-muted-foreground text-xs font-normal"
                                    >(optional)</span
                                >
                            </Form.Label>
                            <Input
                                {...props}
                                bind:value={
                                    $formData.settings.tls.reality_sid
                                }
                                placeholder="Short ID"
                            />
                        {/snippet}
                    </Form.Control>
                    <Form.FieldErrors />
                </Form.Field>
            </div>
        </div>
    {/if}
</div>
