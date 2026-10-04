import type { OutboundMode } from "./types";

export const DEFAULT_POLICY_TAG = "root-policy";

export const RESERVED_TAGS = ["direct", "block", "dns-out"] as const;

// Must be https. The core discards any `http://` delay-test URL and silently
// substitutes `https://www.gstatic.com/generate_204`
// (`experimental/clashapi/proxies.go:191`, `common/urltest/urltest.go:81`), so an
// http URL here is not just ignored — it sends every probe to a Google domain.
export const DELAY_TEST_URL = "https://cp.cloudflare.com/";
export const DELAY_TEST_TIMEOUT_MS = 5000;

export const LATENCY_THRESHOLDS = {
  GOOD: 200,
  FAIR: 350,
} as const;

export const OUTBOUND_MODE_CONFIG: Record<
  OutboundMode,
  { label: string; description: string }
> = {
  direct: {
    label: "Direct Outbound",
    description: "All requests will be sent to the target server directly.",
  },
  global: {
    label: "Global Proxy",
    description: "All requests will be forwarded to a proxy server.",
  },
  rule: {
    label: "Rule-based Proxy",
    description: "Using rule system to determine how to process requests.",
  },
} as const;

export const DEFAULT_FALLBACK_TOLERANCE_MS = 5000;

export const LOG_POLL_INTERVAL_MS = 1000;
export const LOG_DEFAULT_LIMIT = 200;
