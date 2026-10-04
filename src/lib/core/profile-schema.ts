import { z } from "zod";
import { PROFILE_ICON_NAMES } from "./profile-icons";

export const profileFormSchema = z.object({
  // Optional: if left blank, the backend falls back to the subscription's
  // profile-title header, then to a generated name.
  name: z
    .string()
    .max(100, "Name is too long")
    .optional()
    .transform((v) => (v?.trim() ? v.trim() : undefined)),
  url: z
    .union([
      z.string().url("Must be a valid URL"),
      z.string().length(0),
      z.null(),
      z.undefined(),
    ])
    .optional()
    .transform((v) => (v === "" ? undefined : v)),
  skipAutoUpdate: z.boolean().default(false),
  updateIntervalHours: z.coerce
    .number()
    .int()
    .min(1, "At least 1 hour")
    .max(720, "Too large")
    .default(12),
  icon: z.enum(PROFILE_ICON_NAMES).nullable().optional(),
});

export type ProfileFormSchema = z.infer<typeof profileFormSchema>;
