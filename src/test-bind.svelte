<script lang="ts">
  let { triggerAdd = $bindable() } = $props<{ triggerAdd?: () => void }>();
  $effect(() => { triggerAdd = () => { console.log('test') } });
</script>
