import { z } from "zod";

export const ruleFormSchema = z.object({
  kind: z.enum([
    "DOMAIN",
    "DOMAIN-SUFFIX",
    "DOMAIN-KEYWORD",
    "DOMAIN-REGEX",
    "IP-CIDR",
    "IN-PORT",
    "DEST-PORT",
    "GEOIP",
    "GEOSITE",
    "PROCESS-NAME",
    "PROCESS-PATH",
    "PROCESS-PATH-REGEX",
    "PROTOCOL",
    "FINAL",
  ]),
  value: z.string(),
  outbound: z.string().min(1, "Outbound is required"),
  comment: z.string().optional().default(""),
  /** Absolute unix ms; required when creating temp rules. */
  expires_at: z.number().optional(),
});

export type RuleFormSchema = z.infer<typeof ruleFormSchema>;
