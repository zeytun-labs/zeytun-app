import { toast, type ExternalToast } from "svelte-sonner";
import { errorMessage } from "$lib/errors";
import { writeText } from "@tauri-apps/plugin-clipboard-manager";

export interface ToastOptions extends ExternalToast {
  copyable?: boolean;
}

export interface ErrorToastOptions extends ToastOptions {
  title?: string;
}

export const appToast = {
  success(message: string, options?: ToastOptions) {
    return toast.success(message, options);
  },

  error(errOrTitle: unknown, options?: ErrorToastOptions) {
    const rawMsg = errorMessage(errOrTitle);
    let title = options?.title;
    let description = options?.description;

    if (!title) {
      if (typeof errOrTitle === "string" && errOrTitle.length <= 60 && !description) {
        title = errOrTitle;
      } else {
        title = typeof errOrTitle === "string" && !description ? "Error" : "Operation failed";
        description = description || rawMsg;
      }
    } else if (!description && rawMsg && rawMsg !== title) {
      description = rawMsg;
    }

    const toastOpts: ExternalToast = { ...options, description };

    if (!toastOpts.action && (options?.copyable || (description && description.length > 40))) {
      const copyPayload = description ? `${title}: ${description}` : title;
      toastOpts.action = {
        label: "Copy",
        onClick: async () => {
          try {
            await writeText(copyPayload);
            toast.success("Copied to clipboard");
          } catch {
            // ignore clipboard failure
          }
        },
      };
    }

    return toast.error(title, toastOpts);
  },

  warning(message: string, options?: ToastOptions) {
    return toast.warning(message, options);
  },

  info(message: string, options?: ToastOptions) {
    return toast.info(message, options);
  },
};
