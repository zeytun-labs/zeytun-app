import { z } from "zod";
import { PROFILE_ICON_NAMES } from "./profile-icons";

export const profileFormSchema = z.object({
  name: z
    .string()
    .trim()
    .min(1, "Name is required")
    .max(100, "Name is too long"),
  icon: z.enum(PROFILE_ICON_NAMES).nullable().optional(),
});

export type ProfileFormSchema = z.infer<typeof profileFormSchema>;
