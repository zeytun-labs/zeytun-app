<script lang="ts">
  import * as ContextMenu from "$lib/components/ui/context-menu/index.js";
  import PolicyCard from "./policy-card.svelte";
  import type { ProxyPolicy } from "$lib/core/types";
  import { profileStore } from "$lib/stores/profile.svelte";
  import { DEFAULT_POLICY_TAG } from "$lib/core/constants";
  import { HugeiconsIcon } from "@hugeicons/svelte";
  import {
    CheckIcon,
    Delete01Icon,
    Edit01Icon,
  } from "@hugeicons/core-free-icons";

  interface Props {
    policies: ProxyPolicy[];
    canSelectProxy: boolean;
    groupName: string;
    onPolicyClick: (policy: ProxyPolicy) => void;
    onEditPolicy: (policy: ProxyPolicy) => void;
    onSelectPolicy: (tag: string) => void;
    onSelectMember: (policyTag: string, memberTag: string) => void;
    onDeletePolicy: (policy: ProxyPolicy) => void;
  }

  let {
    policies,
    canSelectProxy,
    groupName,
    onPolicyClick,
    onEditPolicy,
    onSelectPolicy,
    onSelectMember,
    onDeletePolicy,
  }: Props = $props();

  function isPolicySelected(policyTag: string) {
    if (!canSelectProxy) return false;
    const defaultPolicy = profileStore.getDefaultPolicy();
    return defaultPolicy?.selected_member_tag === policyTag;
  }
</script>

{#if policies.length > 0}
  <div class="flex flex-col gap-2">
    <p class="text-muted-foreground text-xs tracking-wide">
      POLICY
    </p>
    <div class="grid gap-3 grid-cols-2 lg:grid-cols-4">
      {#each policies as policy (policy.tag)}
        <ContextMenu.Root>
          <ContextMenu.Trigger>
            {#snippet child({ props })}
              <PolicyCard
                {policy}
                isSelected={isPolicySelected(policy.tag)}
                canSelect={canSelectProxy}
                onclick={() => onPolicyClick(policy)}
                triggerProps={props}
              />
            {/snippet}
          </ContextMenu.Trigger>
          <ContextMenu.Content>
            <ContextMenu.Item
              disabled={!canSelectProxy}
              onclick={() => onSelectPolicy(policy.tag)}
            >
              <HugeiconsIcon icon={CheckIcon} />
              Select
            </ContextMenu.Item>
            {#if (policy.kind ?? "selector") === "selector" && profileStore.getPolicyMembers(policy).length > 0}
              <ContextMenu.Separator />
              {#each profileStore.getPolicyMembers(policy) as member (member.tag)}
                <ContextMenu.Item
                  onclick={() => onSelectMember(policy.tag, member.tag)}
                >
                  {#if policy.selected_member_tag === member.tag}
                    <HugeiconsIcon icon={CheckIcon} />
                  {:else}
                    <span class="size-4"></span>
                  {/if}
                  <span class="max-w-48 truncate">{member.name}</span>
                </ContextMenu.Item>
              {/each}
            {/if}
            <ContextMenu.Separator />
            <ContextMenu.Item
              disabled={policy.tag === DEFAULT_POLICY_TAG}
              onclick={() => onEditPolicy(policy)}
            >
              <HugeiconsIcon icon={Edit01Icon} />
              Edit
            </ContextMenu.Item>
            <ContextMenu.Item
              variant="destructive"
              disabled={policy.tag === DEFAULT_POLICY_TAG}
              onclick={() => onDeletePolicy(policy)}
            >
              <HugeiconsIcon icon={Delete01Icon} />
              Delete
            </ContextMenu.Item>
          </ContextMenu.Content>
        </ContextMenu.Root>
      {/each}
    </div>
  </div>
{/if}
