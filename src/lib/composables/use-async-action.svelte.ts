import { appToast } from "$lib/toast";

export function useAsyncAction() {
  let saving = $state(false);

  async function run(action: () => Promise<void>, successMessage?: string) {
    saving = true;
    try {
      await action();
      if (successMessage) appToast.success(successMessage);
    } catch (err) {
      appToast.error(err);
    } finally {
      saving = false;
    }
  }

  return {
    get saving() {
      return saving;
    },
    run,
  };
}
