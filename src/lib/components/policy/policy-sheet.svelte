<script lang="ts">
  import * as Sheet from "$lib/components/ui/sheet";
  import { Button } from "$lib/components/ui/button";
  import { Input } from "$lib/components/ui/input";
  import * as Form from "$lib/components/ui/form";
  import * as Select from "$lib/components/ui/select";
  import type {
    Proxy,
    ProxyPolicy,
    ProxyPolicyType,
    BalancerStrategy,
  } from "$lib/core/types";
  import { cn, formatLatency, getLatencyColor } from "$lib/utils";
  import { latencyStore } from "$lib/stores/latency.svelte";
  import { Checkbox } from "../ui/checkbox";
  import { ScrollArea } from "../ui/scroll-area";
  import { superForm, defaults } from "sveltekit-superforms";
  import { zod4 } from "sveltekit-superforms/adapters";
  import {
    policyFormSchema,
    type PolicyFormSchema,
  } from "$lib/core/policy-schema.js";

  interface Props {
    open: boolean;
    mode?: "create" | "edit";
    policy?: ProxyPolicy | null;
    proxies: Proxy[];
    saving: boolean;
    onSave: (
      name: string,
      type: ProxyPolicyType,
      members: string[],
      strategy?: string | null,
      tolerance_ms?: number | null,
      weights?: number[] | null,
    ) => void;
  }

  let {
    open = $bindable(false),
    mode = "create",
    policy = null,
    proxies,
    saving,
    onSave,
  }: Props = $props();

  let memberWeights = $state<Record<string, number>>({});

  const policyTypeLabels: Record<ProxyPolicyType | BalancerStrategy, string> = {
    selector: "Manual",
    urltest: "Auto",
    balancer: "Balancer", // Kept for legacy/typing but not rendered in options directly
    "round-robin": "Round Robin",
    "consistent-hashing": "Consistent Hashing",
    "sticky-sessions": "Sticky Sessions",
    failover: "Failover",
    weighted: "Weighted",
    "least-connections": "Least Connections",
  };

  const strategyDescriptions: Record<string, string> = {
    "round-robin": "Distributes requests sequentially among available members.",
    "consistent-hashing": "Consistently routes identical requests to the same member based on hashing.",
    "sticky-sessions": "Maintains session consistency by pinning connections to the same member.",
    failover: "Uses the first available member in order, switching only if it fails.",
    weighted: "Distributes traffic proportionally based on assigned member weights.",
    "least-connections": "Routes new requests to the member with the fewest active connections.",
  };

  const form = superForm<PolicyFormSchema>(
    { name: "", type: "selector", members: [], strategy: "round-robin" },
    {
      SPA: true,
      validators: zod4(policyFormSchema as any),
      onUpdate({ form: f }) {
        if (f.valid) {
          // Reconstruct the weights array mapping from our state record to match the selected members array.
          let finalWeights: number[] | null = null;
          if (f.data.type === "balancer" && f.data.strategy === "weighted") {
            finalWeights = f.data.members.map((m) => memberWeights[m] || 1);
          }

          onSave(
            f.data.name,
            f.data.type as ProxyPolicyType,
            f.data.members,
            f.data.strategy,
            f.data.tolerance_ms,
            finalWeights,
          );
        }
      },
    },
  );

  const { form: formData, enhance, reset } = form;

  $effect(() => {
    if (open) {
      if (mode === "edit" && policy) {
        // Initialize memberWeights from the policy's parallel arrays
        let initialWeights: Record<string, number> = {};
        if (
          policy.members &&
          policy.weights &&
          policy.weights.length === policy.members.length
        ) {
          policy.members.forEach((m, idx) => {
            initialWeights[m] = policy.weights![idx];
          });
        }
        memberWeights = initialWeights;

        reset({
          data: {
            name: policy.name,
            type: (policy.kind as ProxyPolicyType) ?? "selector",
            members: policy.members ?? [],
            strategy: (policy.strategy as BalancerStrategy) ?? "round-robin",
            tolerance_ms: policy.tolerance_ms ?? null,
            weights: policy.weights ?? null,
          },
        });
      } else {
        memberWeights = {};
        reset({
          data: {
            name: "",
            type: "selector",
            members: [],
            strategy: "round-robin",
            tolerance_ms: null,
            weights: null,
          },
        });
      }
    }
  });

  const enabledProxies = $derived(proxies.filter((p) => p.enabled));

  function toggleMember(proxyId: string) {
    if ($formData.members.includes(proxyId)) {
      $formData.members = $formData.members.filter((id) => id !== proxyId);
      delete memberWeights[proxyId];
    } else {
      $formData.members = [...$formData.members, proxyId];
      if (!(proxyId in memberWeights)) {
        memberWeights[proxyId] = 1;
      }
    }
  }
</script>

<Sheet.Root bind:open>
  <Sheet.Content
    side="right"
    class="flex flex-col gap-0 px-1 transition-all duration-300 flex-1 w-3xl! max-w-3xl!"
  >
    <form use:enhance class="flex flex-col h-full">
      <div class="flex flex-col flex-1 h-0 min-h-0">
        <Sheet.Header class="px-3">
          <Sheet.Title
            >{mode === "edit" ? "Edit Policy" : "Add new Policy"}</Sheet.Title
          >
          <Sheet.Description>
            {mode === "edit"
              ? "Edit the existing policy settings and members."
              : "Create a new policy to organize proxy routing within this group."}
          </Sheet.Description>
        </Sheet.Header>

        <div class="gap-6 px-3 flex-1 min-h-0 grid grid-cols-2">
          <div class="flex flex-col gap-3 flex-1">
            <Form.Field {form} name="name">
              <Form.Control>
                {#snippet children({ props })}
                  <Form.Label>Name</Form.Label>
                  <Input
                    {...props}
                    bind:value={$formData.name}
                    placeholder="Streaming Policy"
                  />
                {/snippet}
              </Form.Control>
              <Form.FieldErrors />
            </Form.Field>

            <Form.Field {form} name="type">
              <Form.Control>
                {#snippet children({ props })}
                  <Form.Label>Type</Form.Label>
                  <Select.Root
                    type="single"
                    name={props.name}
                    value={$formData.type === "balancer"
                      ? $formData.strategy
                      : $formData.type}
                    onValueChange={(v) => {
                      if (
                        v === "round-robin" ||
                        v === "consistent-hashing" ||
                        v === "sticky-sessions" ||
                        v === "failover" ||
                        v === "weighted" ||
                        v === "least-connections"
                      ) {
                        $formData.type = "balancer";
                        $formData.strategy = v as BalancerStrategy;
                      } else {
                        $formData.type = v as any;
                      }
                    }}
                  >
                    <Select.Trigger class="w-full justify-between" {...props}>
                      {policyTypeLabels[
                        ($formData.type === "balancer"
                          ? $formData.strategy
                          : $formData.type) as keyof typeof policyTypeLabels
                      ] || "Select type"}
                    </Select.Trigger>
                    <Select.Content>
                      <Select.Group>
                        <Select.Label>Basic Types</Select.Label>
                        <Select.Item
                          value="selector"
                          label={policyTypeLabels.selector}
                        >
                          {policyTypeLabels.selector}
                        </Select.Item>
                        <Select.Item
                          value="urltest"
                          label={policyTypeLabels.urltest}
                        >
                          {policyTypeLabels.urltest}
                        </Select.Item>
                        <Select.Separator />
                        <Select.Label>Balancer Strategies</Select.Label>
                        {#each ["round-robin", "consistent-hashing", "sticky-sessions", "failover", "weighted", "least-connections"] as s}
                          <Select.Item
                            value={s}
                            label={policyTypeLabels[s as BalancerStrategy]}
                          >
                            {policyTypeLabels[s as BalancerStrategy]}
                          </Select.Item>
                        {/each}
                      </Select.Group>
                    </Select.Content>
                  </Select.Root>
                {/snippet}
              </Form.Control>
              {#if $formData.type === "selector"}
                <Form.Description class="pb-2"
                  >Choose which policy to use in the main menu or policy view.</Form.Description
                >
              {:else if $formData.type === "urltest"}
                <Form.Description class="pb-2"
                  >Automatically select which policy will be used by
                  benchmarking the latency to the testing URL.</Form.Description
                >
                <Form.Field {form} name="tolerance_ms">
                  <Form.Control>
                    {#snippet children({ props })}
                      <Form.Label>Tolerance (ms)</Form.Label>
                      <Input
                        {...props}
                        type="number"
                        value={$formData.tolerance_ms || ""}
                        onchange={(e) => {
                          const val = parseInt(e.currentTarget.value);
                          $formData.tolerance_ms = isNaN(val) ? null : val;
                        }}
                        placeholder="e.g. 50"
                      />
                    {/snippet}
                  </Form.Control>
                  <Form.Description class="pb-2"
                    >Latency difference band for switching. Empty = 50ms.</Form.Description
                  >
                  <Form.FieldErrors />
                </Form.Field>
              {:else if $formData.type === "balancer"}
                <Form.Description class="pb-2">
                  {strategyDescriptions[$formData.strategy || "round-robin"] ||
                    "Distribute traffic among available members using a specific strategy."}
                </Form.Description>
                <Form.Field {form} name="tolerance_ms">
                  <Form.Control>
                    {#snippet children({ props })}
                      <Form.Label>Tolerance (ms)</Form.Label>
                      <Input
                        {...props}
                        type="number"
                        value={$formData.tolerance_ms || ""}
                        onchange={(e) => {
                          const val = parseInt(e.currentTarget.value);
                          $formData.tolerance_ms = isNaN(val) ? null : val;
                        }}
                        placeholder="e.g. 50"
                      />
                    {/snippet}
                  </Form.Control>
                  <Form.Description
                    >Latency difference band for switching. Empty = 50ms.</Form.Description
                  >
                  <Form.FieldErrors />
                </Form.Field>

                {#if $formData.strategy === "weighted"}
                  <!-- Global Weights field removed, weights are now per-member -->
                {/if}
              {:else}
                <Form.Description
                  >Automatically select an available policy by priority. The
                  availability is tested by accessing the testing URL.</Form.Description
                >
              {/if}
              <Form.FieldErrors />
            </Form.Field>
          </div>

          <div class="flex-1 flex flex-col h-full pb-2">
            <Form.Field {form} name="members" class="flex flex-col flex-1">
              <Form.Control>
                {#snippet children({ props })}
                  <div class="flex justify-between items-end">
                    <Form.Label>Members</Form.Label>
                    <Form.Description class="pr-2 text-xs"
                      >{$formData.members
                        .length}/{enabledProxies.length}</Form.Description
                    >
                  </div>
                  <ScrollArea
                    class="rounded-lg border border-border/50 flex-1 min-h-0 h-0"
                  >
                    {#each enabledProxies as proxy, i (proxy.tag)}
                      <div
                        class={cn(
                          "w-full flex items-center gap-3 transition-colors hover:bg-muted/30",
                          $formData.members.includes(proxy.tag) &&
                            "bg-muted/20",
                        )}
                      >
                        <button
                          type="button"
                          class="flex flex-1 min-w-0 items-center gap-3 px-3 py-2"
                          onclick={() => toggleMember(proxy.tag)}
                        >
                          <Checkbox
                            checked={$formData.members.includes(proxy.tag)}
                            class="pointer-events-none"
                            onclick={(e) => {
                              e.preventDefault();
                            }}
                          />
                          <div class="flex-1 min-w-0 text-left flex flex-col">
                            <div class="truncate text-sm">{proxy.title}</div>
                            <div class="text-muted-foreground text-xs">
                              {proxy.protocol}
                            </div>
                          </div>
                        </button>

                        {#if $formData.type === "balancer" && $formData.strategy === "weighted" && $formData.members.includes(proxy.tag)}
                          <div class="flex items-center gap-2 py-2 pr-3">
                            <Input
                              type="number"
                              min="1"
                              class="h-6 w-8 px-1 py-0 text-xs text-center"
                              value={memberWeights[proxy.tag] || 1}
                              onclick={(e) => {
                                e.stopPropagation();
                              }}
                              oninput={(e) => {
                                const val =
                                  parseInt(e.currentTarget.value) || 1;
                                memberWeights[proxy.tag] = Math.max(1, val);
                              }}
                            />
                          </div>
                        {/if}
                      </div>
                    {/each}

                    {#if enabledProxies.length === 0}
                      <div
                        class="text-muted-foreground px-3 py-4 text-center text-sm"
                      >
                        No enabled proxies in this group.
                      </div>
                    {/if}
                  </ScrollArea>
                {/snippet}
              </Form.Control>

              <Form.FieldErrors />
            </Form.Field>
          </div>
        </div>
      </div>

      <Sheet.Footer
        class="-mx-1! mt-auto bg-background border-t border-border flex-row justify-end"
      >
        <Button type="button" variant="outline" onclick={() => (open = false)}
          >Cancel</Button
        >
        <Form.Button class="flex-1" disabled={saving}>
          {saving ? "Saving..." : "Save"}
        </Form.Button>
      </Sheet.Footer>
    </form>
  </Sheet.Content>
</Sheet.Root>
