<script lang="ts">
  import * as Dialog from "$lib/components/ui/dialog/index.js";
  import { Button } from "$lib/components/ui/button/index.js";
  import { Input } from "$lib/components/ui/input/index.js";
  import { Label } from "$lib/components/ui/label/index.js";
  import * as Form from "$lib/components/ui/form/index.js";
  import * as Select from "$lib/components/ui/select/index.js";
  import { superForm } from "sveltekit-superforms";
  import { zod4 } from "sveltekit-superforms/adapters";
  import {
    ruleFormSchema,
    type RuleFormSchema,
  } from "$lib/core/rule-schema.js";
  import type { Rule, RuleType, TempRule } from "$lib/core/types";

  export type ActionOption = { tag: string; name: string };
  export type ActionGroup = { label?: string; options: ActionOption[] };

  interface Props {
    open: boolean;
    mode: "create" | "edit";
    rule?: Rule | TempRule | null;
    actionGroups: ActionGroup[];
    getActionName: (tag: string) => string;
    onSave: (data: RuleFormSchema) => void;
    /** When true, require expires_at and hide FINAL. */
    temp?: boolean;
  }

  let {
    open = $bindable(false),
    mode,
    rule = null,
    actionGroups,
    getActionName,
    onSave,
    temp = false,
  }: Props = $props();

  const TTL_PRESETS = [
    { label: "1 hour", ms: 3_600_000 },
    { label: "6 hours", ms: 21_600_000 },
    { label: "24 hours", ms: 86_400_000 },
    { label: "7 days", ms: 604_800_000 },
  ] as const;

  const ruleKinds: { title?: string; types: RuleType[] }[] = [
    {
      title: "Domain Rules",
      types: [
        "DOMAIN",
        "DOMAIN-SUFFIX",
        "DOMAIN-KEYWORD",
        "DOMAIN-REGEX",
        "GEOSITE",
      ],
    },
    {
      title: "IP Rules",
      types: ["IP-CIDR", "GEOIP"],
    },

    {
      title: "Process Rules",
      types: ["PROCESS-NAME", "PROCESS-PATH", "PROCESS-PATH-REGEX"],
    },
    {
      title: "Port Rules",
      types: ["IN-PORT", "DEST-PORT"],
    },
    {
      title: "Other Rules",
      types: ["PROTOCOL"],
    },
  ];

  const form = superForm<RuleFormSchema>(
    {
      kind: "DOMAIN-SUFFIX",
      value: "",
      outbound: "direct",
      comment: "",
      expires_at: undefined,
    },
    {
      SPA: true,
      validators: zod4(ruleFormSchema as any),
      onUpdate({ form: f }) {
        if (f.valid) {
          // expires_at === 0 = Session rule (no expiry, dies on reload).
          if (
            temp &&
            f.data.expires_at !== 0 &&
            (!f.data.expires_at || f.data.expires_at <= Date.now())
          ) {
            return;
          }
          onSave(f.data);
        }
      },
    },
  );

  const { form: formData, enhance, reset } = form;

  $effect(() => {
    if (!open) return;
    if (mode === "edit" && rule) {
      reset({
        data: {
          kind: rule.kind,
          value: rule.value,
          outbound: rule.outbound,
          comment: rule.comment || "",
          expires_at:
            temp && "expires_at" in rule ? rule.expires_at : undefined,
        },
      });
    } else {
      const defaultOutbound = actionGroups[0]?.options[0]?.tag || "direct";
      reset({
        data: {
          kind: "DOMAIN-SUFFIX",
          value: "",
          outbound: defaultOutbound,
          comment: "",
          expires_at: temp ? Date.now() + 3_600_000 : undefined,
        },
      });
    }
  });

  function expiresLocalValue(ms?: number) {
    if (!ms) return "";
    const d = new Date(ms);
    const pad = (n: number) => String(n).padStart(2, "0");
    return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}T${pad(d.getHours())}:${pad(d.getMinutes())}`;
  }
  let currentActionGroups = $derived(
    $formData.kind === "FINAL"
      ? [
          {
            options: [
              { tag: "direct", name: "Direct" },
              { tag: "root-policy", name: "Global Proxy" },
            ],
          },
        ]
      : actionGroups,
  );

  const getLocalActionName = (tag: string) => {
    if ($formData.kind === "FINAL" && tag === "root-policy") return "Global Proxy";
    return getActionName(tag);
  };
</script>

<Dialog.Root bind:open>
  <Dialog.Content class="sm:max-w-lg">
    <Dialog.Header>
      <Dialog.Title
        >{mode === "create"
          ? temp
            ? "Add temporary rule"
            : "Add new Rule"
          : temp
            ? "Edit temporary rule"
            : "Edit Rule"}</Dialog.Title
      >
      <Dialog.Description>
        {temp
          ? "Matches until expiry, or until the core/profile reloads for session rules."
          : "Configure the routing rule settings."}
      </Dialog.Description>
    </Dialog.Header>

    <form use:enhance>
      <div class="grid gap-4 py-4">
        <Form.Field {form} name="kind">
          <Form.Control>
            {#snippet children({ props })}
              <Form.Label>Type</Form.Label>
              <Select.Root
                disabled={$formData.kind === "FINAL"}
                type="single"
                name={props.name}
                value={$formData.kind}
                onValueChange={(v) => {
                  $formData.kind = v as any;
                }}
              >
                <Select.Trigger class="w-full justify-between" {...props}>
                  {$formData.kind}
                </Select.Trigger>
                <Select.Content class="max-h-64">
                  {#each ruleKinds as kindGroup, index (index)}
                    <Select.Group>
                      {#if !!kindGroup.title}
                        <Select.Label>{kindGroup.title}</Select.Label>
                      {/if}
                      {#each kindGroup.types as item, i (i)}
                        <Select.Item value={item} label={item}>
                          {item}
                        </Select.Item>
                      {/each}
                    </Select.Group>
                  {/each}
                </Select.Content>
              </Select.Root>
            {/snippet}
          </Form.Control>
          <Form.FieldErrors />
        </Form.Field>

        {#if $formData.kind !== "FINAL"}
        <Form.Field {form} name="value">
          <Form.Control>
            {#snippet children({ props })}
              <Form.Label>Value</Form.Label>
              <Input
                {...props}
                bind:value={$formData.value}
                placeholder="example.com"
              />
            {/snippet}
          </Form.Control>
          <Form.FieldErrors />
        </Form.Field>
        {/if}

        <Form.Field {form} name="outbound">
          <Form.Control>
            {#snippet children({ props })}
              <Form.Label>Action</Form.Label>
              <Select.Root
                type="single"
                name={props.name}
                value={$formData.outbound}
                onValueChange={(v) => {
                  $formData.outbound = v as any;
                }}
              >
                <Select.Trigger class="w-full justify-between" {...props}>
                  {getLocalActionName($formData.outbound)}
                </Select.Trigger>
                <Select.Content class="max-h-64">
                  {#each currentActionGroups as group}
                    {#if group.options.length > 0}
                      <Select.Group>
                        {#if group.label}
                          <Select.Label>{group.label}</Select.Label>
                        {/if}
                        {#each group.options as option (option.tag)}
                          <Select.Item value={option.tag} label={option.name}>
                            {option.name}
                          </Select.Item>
                        {/each}
                      </Select.Group>
                    {/if}
                  {/each}
                </Select.Content>
              </Select.Root>
            {/snippet}
          </Form.Control>
          <Form.FieldErrors />
        </Form.Field>

        {#if temp}
          <div class="grid gap-2">
            <Label>Expires</Label>
            <div class="flex flex-wrap gap-2">
              {#each TTL_PRESETS as p}
                <Button
                  type="button"
                  size="sm"
                  variant="outline"
                  onclick={() => {
                    $formData.expires_at = Date.now() + p.ms;
                  }}
                >
                  {p.label}
                </Button>
              {/each}
              <Button
                type="button"
                size="sm"
                variant={$formData.expires_at === 0 ? "default" : "outline"}
                onclick={() => {
                  $formData.expires_at = 0;
                }}
              >
                Session
              </Button>
            </div>
            {#if $formData.expires_at !== 0}
              <Input
                type="datetime-local"
                value={expiresLocalValue($formData.expires_at)}
                oninput={(e) => {
                  const v = (e.currentTarget as HTMLInputElement).value;
                  $formData.expires_at = v ? new Date(v).getTime() : undefined;
                }}
              />
            {/if}
            {#if $formData.expires_at === 0}
              <p class="text-muted-foreground text-xs">
                Disappears after reloading the core/profile or changing policies.
              </p>
            {/if}
            {#if $formData.expires_at && $formData.expires_at <= Date.now()}
              <p class="text-destructive text-xs">Expiry must be in the future.</p>
            {/if}
          </div>
        {/if}

        <Form.Field {form} name="comment">
          <Form.Control>
            {#snippet children({ props })}
              <Form.Label>
                Comment
                <span class="text-muted-foreground text-xs font-normal"
                  >(optional)</span
                >
              </Form.Label>
              <Input
                {...props}
                bind:value={$formData.comment}
                placeholder="My custom rule"
              />
            {/snippet}
          </Form.Control>
          <Form.FieldErrors />
        </Form.Field>
      </div>

      <Dialog.Footer>
        <Button type="button" variant="outline" onclick={() => (open = false)}
          >Cancel</Button
        >
        <Form.Button>Save</Form.Button>
      </Dialog.Footer>
    </form>
  </Dialog.Content>
</Dialog.Root>
