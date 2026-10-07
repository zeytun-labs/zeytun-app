<script lang="ts">
  import * as DropdownMenu from "$lib/components/ui/dropdown-menu/index.js";
  import * as AlertDialog from "$lib/components/ui/alert-dialog/index.js";
  import { HugeiconsIcon } from "@hugeicons/svelte";
  import {
    Add01Icon,
    Delete01Icon,
    Download01Icon,
    Edit03Icon,
    Notification03Icon,
    UnfoldMoreIcon,
    Upload01Icon,
  } from "@hugeicons/core-free-icons";

  import { profileStore } from "$lib/stores/profile.svelte";
  import { useAsyncAction } from "$lib/composables/use-async-action.svelte";
  import { cn, formatBytes } from "$lib/utils";
  import { getProfileIcon } from "$lib/core/profile-icons";
  import type { ProfileMeta, SyncSummary } from "$lib/core/types";
  import ProfileDialog, {
    type ProfileDialogValues,
  } from "./profile/profile-dialog.svelte";
  import SyncSummaryDialog from "./profile/sync-summary-dialog.svelte";
  import ProfileImportDialog from "./profile/profile-import-dialog.svelte";
  import ProfileExportDialog from "./profile/profile-export-dialog.svelte";

  let { compact = false }: { compact?: boolean } = $props();

  const action = useAsyncAction();

  // Dialog state
  type DialogKind = "create" | "edit";
  let dialogOpen = $state(false);
  let dialogKind = $state<DialogKind>("create");
  let editTarget = $state<ProfileMeta | null>(null);

  // Import / Export state
  let importOpen = $state(false);
  let exportOpen = $state(false);
  let exportTarget = $state<ProfileMeta | null>(null);

  // Sync-summary modal
  let summaryOpen = $state(false);
  let summaryData = $state<SyncSummary | null>(null);
  let summaryName = $state("");

  // Delete confirmation
  let deleteOpen = $state(false);
  let deleteTarget = $state<ProfileMeta | null>(null);

  const active = $derived(profileStore.activeProfile);
  const profiles = $derived(profileStore.profiles);
  const canDelete = $derived(profiles.length > 1);

  async function switchTo(p: ProfileMeta) {
    if (p.is_active) return;
    await action.run(async () => {
      await profileStore.switchProfile(p.id);
    });
  }

  function openCreate() {
    dialogKind = "create";
    editTarget = null;
    dialogOpen = true;
  }

  function openEdit(p: ProfileMeta) {
    dialogKind = "edit";
    editTarget = p;
    dialogOpen = true;
  }

  async function saveProfile(values: ProfileDialogValues) {
    if (dialogKind === "create") {
      await action.run(async () => {
        await profileStore.createProfile({
          name: values.name ?? null,
          icon: values.icon ?? null,
        });
        dialogOpen = false;
      }, "Profile created.");
    } else if (editTarget) {
      const id = editTarget.id;
      await action.run(async () => {
        await profileStore.updateProfile({
          id,
          name: values.name ?? null,
          icon: values.icon ?? null,
        });
        dialogOpen = false;
      }, "Profile updated.");
    }
  }

  async function viewSummary(p: ProfileMeta) {
    summaryData = p.last_sync_summary ?? null;
    summaryName = p.name;
    summaryOpen = true;
    if (p.unread_summary) {
      await profileStore.markSummaryRead(p.id);
    }
  }

  function confirmDelete(p: ProfileMeta) {
    deleteTarget = p;
    deleteOpen = true;
  }

  async function doDelete() {
    if (!deleteTarget) return;
    const id = deleteTarget.id;
    await action.run(async () => {
      await profileStore.deleteProfile(id);
      deleteOpen = false;
      deleteTarget = null;
    }, "Profile deleted.");
  }
</script>

{#snippet notifyDot()}
  <span class="bg-primary inline-flex size-2 shrink-0 rounded-full"></span>
{/snippet}

{#snippet profileAvatar(p: ProfileMeta | null, size: "sm" | "lg" = "lg")}
  {@const icon = getProfileIcon(p?.icon)}
  <div
    class={cn(
      "bg-muted border flex shrink-0 items-center justify-center font-medium overflow-hidden rounded-full",
      size === "lg" ? "size-8 text-xs" : "size-6 text-[10px]",
    )}
  >
    {#key p?.icon}
      {#if icon}
        <HugeiconsIcon {icon} class={size === "lg" ? "size-4" : "size-3"} />
      {:else}
        <p class="translate-y-[0.1em]">
          {(p?.name ?? "?").charAt(0).toUpperCase()}
        </p>
      {/if}
    {/key}
  </div>
{/snippet}

<DropdownMenu.Root>
  <DropdownMenu.Trigger>
    {#snippet child({ props })}
      <button
        {...props}
        type="button"
        class={cn(
          "flex items-center gap-2 rounded-full text-start outline-none transition-colors",
          "data-[state=open]:bg-accent data-[state=open]:text-accent-foreground",
          "focus-visible:ring-2 focus-visible:ring-ring",
          compact
            ? "size-11 justify-center border border-border/40 bg-card/85 shadow-lg backdrop-blur-xl hover:bg-accent"
            : "h-fit w-full rounded-2xl border bg-card py-2 px-3",
        )}
      >
        {@render profileAvatar(active, "lg")}
      </button>
    {/snippet}
  </DropdownMenu.Trigger>
  <DropdownMenu.Content
    class="min-w-56 w-(--bits-dropdown-menu-anchor-width)"
    side="right"
    align="end"
    sideOffset={16}
  >
        <DropdownMenu.Group>
          <DropdownMenu.Item>
            {@render profileAvatar(active)}
            <div
              class="grid flex-1 min-w-0 text-start text-sm leading-tight gap-1"
            >
              <div class="flex items-center justify-between gap-2">
                <span class="truncate font-medium">
                  {active?.name ?? "No profile"}
                </span>
              </div>
              <div
                class="text-muted-foreground flex items-center justify-between text-xs truncate mt-px"
              >
                {active ? "Active Profile" : ""}
              </div>
            </div>
          </DropdownMenu.Item>
        </DropdownMenu.Group>

        <DropdownMenu.Separator />

        <!-- Profile list: click to switch -->
        <DropdownMenu.Group>
          {#each profiles.filter((p) => !p.is_active) as p (p.id)}
            <DropdownMenu.Item onclick={() => switchTo(p)}>
              {@render profileAvatar(p, "sm")}
              <span class="truncate">{p.name}</span>
            </DropdownMenu.Item>
          {/each}
        </DropdownMenu.Group>

        <DropdownMenu.Separator />
        <DropdownMenu.Group>
          <DropdownMenu.Item onclick={openCreate}>
            <HugeiconsIcon icon={Add01Icon} />
            Create new Profile
          </DropdownMenu.Item>
          <DropdownMenu.Item onclick={() => (importOpen = true)}>
            <HugeiconsIcon icon={Upload01Icon} />
            Import Profile
          </DropdownMenu.Item>
        </DropdownMenu.Group>

        {#if active}
          <DropdownMenu.Separator />
          <DropdownMenu.Group>
            {#if active.unread_summary}
              <DropdownMenu.Item onclick={() => viewSummary(active)}>
                <HugeiconsIcon icon={Notification03Icon} />
                View Update Summary
                <span class="ml-auto">{@render notifyDot()}</span>
              </DropdownMenu.Item>
            {/if}
            <DropdownMenu.Item
              onclick={() => {
                exportTarget = active;
                exportOpen = true;
              }}
            >
              <HugeiconsIcon icon={Download01Icon} />
              Export Profile
            </DropdownMenu.Item>
            <DropdownMenu.Item onclick={() => openEdit(active)}>
              <HugeiconsIcon icon={Edit03Icon} />
              Edit
            </DropdownMenu.Item>
          </DropdownMenu.Group>

          <DropdownMenu.Separator />
          <DropdownMenu.Group>
            <DropdownMenu.Item
              variant="destructive"
              disabled={!canDelete}
              onclick={() => confirmDelete(active)}
            >
              <HugeiconsIcon icon={Delete01Icon} />
              Delete
            </DropdownMenu.Item>
          </DropdownMenu.Group>
        {/if}
  </DropdownMenu.Content>
</DropdownMenu.Root>

<ProfileDialog
  bind:open={dialogOpen}
  mode={dialogKind}
  initialName={dialogKind === "edit" ? (editTarget?.name ?? "") : ""}
  initialIcon={dialogKind === "edit" ? (editTarget?.icon ?? null) : null}
  saving={action.saving}
  onSave={saveProfile}
/>

<ProfileImportDialog bind:open={importOpen} />

<ProfileExportDialog bind:open={exportOpen} profile={exportTarget} />

<SyncSummaryDialog
  bind:open={summaryOpen}
  profileName={summaryName}
  summary={summaryData}
  onClose={() => (summaryOpen = false)}
/>

<AlertDialog.Root bind:open={deleteOpen}>
  <AlertDialog.Content>
    <AlertDialog.Header>
      <AlertDialog.Title>Delete profile?</AlertDialog.Title>
      <AlertDialog.Description>
        This permanently removes "{deleteTarget?.name}" and all its proxies,
        policies, and rules. This cannot be undone.
      </AlertDialog.Description>
    </AlertDialog.Header>
    <AlertDialog.Footer>
      <AlertDialog.Cancel>Cancel</AlertDialog.Cancel>
      <AlertDialog.Action
        onclick={doDelete}
        class="bg-destructive text-destructive-foreground hover:bg-destructive/90"
      >
        Delete
      </AlertDialog.Action>
    </AlertDialog.Footer>
  </AlertDialog.Content>
</AlertDialog.Root>
