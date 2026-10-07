import { z } from "zod";
import { PROFILE_ICON_NAMES } from "./profile-icons";

export const profileFormSchema = z.object({
  name: z
    .string()
    .max(100, "Name is too long")
    .optional()
    .transform((v) => (v?.trim() ? v.trim() : undefined)),
  icon: z.enum(PROFILE_ICON_NAMES).nullable().optional(),
});

export type ProfileFormSchema = z.infer<typeof profileFormSchema>;
