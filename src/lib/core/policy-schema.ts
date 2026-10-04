import { z } from "zod";

export const policyFormSchema = z.object({
  name: z.string().min(1, "Name is required").max(100, "Name is too long"),
  type: z.enum(["selector", "urltest", "balancer"]),
  members: z.array(z.string()).min(1, "At least one member must be selected"),
  
  strategy: z.enum([
    "round-robin",
    "consistent-hashing",
    "sticky-sessions",
    "failover",
    "weighted",
    "least-connections",
  ]).optional(),
  tolerance_ms: z.number().optional().nullable(),
  delay_acceptable_ratio: z.number().optional().nullable(),
  max_retry: z.number().optional().nullable(),
  ttl: z.string().optional().nullable(),
  weights: z.array(z.number()).optional().nullable(),
});

export type PolicyFormSchema = z.infer<typeof policyFormSchema>;
