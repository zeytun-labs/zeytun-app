import { toast } from "svelte-sonner";
import { errorMessage } from "$lib/errors";

export function useAsyncAction() {
  let saving = $state(false);

  async function run(action: () => Promise<void>, successMessage?: string) {
    saving = true;
    try {
      await action();
      if (successMessage) toast.success(successMessage);
    } catch (err) {
      toast.error(errorMessage(err));
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
