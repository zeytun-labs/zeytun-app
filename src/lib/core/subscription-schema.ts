import { z } from "zod";

export const subscriptionFormSchema = z.object({
  name: z.string().trim().min(1, "Name is required").max(100, "Name is too long"),
  url: z
    .string()
    .trim()
    .min(1, "URL is required")
    .url("Must be a valid URL")
    .refine((u) => u.startsWith("http://") || u.startsWith("https://"), {
      message: "URL must start with http:// or https://",
    }),
  enabled: z.boolean().default(true),
  auto_update: z.boolean().default(true),
  update_interval_hours: z.coerce
    .number()
    .int()
    .min(1, "At least 1 hour")
    .max(720, "Too large")
    .default(24),
});

export type SubscriptionFormData = z.infer<typeof subscriptionFormSchema>;
