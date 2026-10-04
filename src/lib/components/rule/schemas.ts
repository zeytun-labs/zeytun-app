import { z } from "zod";
export const schema = z.object({
  id: z.number(),
  kind: z.string(),
  value: z.string(),
  outbound: z.string(),
  comment: z.string(),
  rule_set: z.string().optional().nullable(),
  orphaned: z.boolean().optional(),
  expires_at: z.number().optional(),
  enabled: z.boolean().optional(),
  session: z.boolean().optional(),
  ask_group_key: z.string().optional(),
});
export type Schema = z.infer<typeof schema>;