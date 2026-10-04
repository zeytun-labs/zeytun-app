<script lang="ts">
  import ConnectionAskDialog from "$lib/components/connection-ask-dialog.svelte";
  import { themePreference } from "$lib/theme.svelte.js";
  import { onMount } from "svelte";
  import { profileStore } from "$lib/stores/profile.svelte";

  onMount(() => {
    themePreference.init();
    void profileStore.startListening();
    void profileStore.load();

    return () => {
      profileStore.stopListening();
    };
  });
</script>

<div
  class="h-screen w-screen overflow-hidden bg-transparent font-sans antialiased text-foreground"
>
  <ConnectionAskDialog />
</div>
