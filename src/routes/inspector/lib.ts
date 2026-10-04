import type { ConnDto, RuleType } from "$lib/core/types";

export type Tab = "active" | "recent";
export type GroupBy = "client" | "host";

/** The host (or address) without a trailing port. */
export function hostOf(r: ConnDto): string {
  return (r.host || r.address).replace(/:\d+$/, "");
}

export function isIpv4(host: string): boolean {
  return /^\d{1,3}(\.\d{1,3}){3}$/.test(host);
}

/** Active connections have no meaningful elapsed time → "-". */
export function formatDuration(r: ConnDto): string {
  if (r.status === "active") return "—";
  const ms = r.durationMs;
  if (ms < 1000) return `${ms}ms`;
  const s = ms / 1000;
  if (s < 60) return `${Math.round(s)}s`;
  const m = Math.floor(s / 60);
  return `${m}m ${Math.round(s % 60)}s`;
}

/** Application protocol / "method"; zeytun-core leaves `protocol` empty often, so
 *  fall back to inferring from the destination port. */
export function methodOf(r: ConnDto): string {
  if (r.protocol) return r.protocol.toUpperCase();
  const port = r.address.includes(":")
    ? r.address.slice(r.address.lastIndexOf(":") + 1)
    : "";
  switch (port) {
    case "443":
      return "HTTPS";
    case "80":
      return "HTTP";
    case "53":
      return "DNS";
    case "22":
      return "SSH";
    default:
      return (r.network || "—").toUpperCase();
  }
}

export function statusColor(status: ConnDto["status"]): string {
  switch (status) {
    case "active":
      return "bg-amber-500";
    case "error":
      return "bg-rose-500";
    default:
      return "bg-lime-500";
  }
}

export function fmtTime(epochSecs: number): string {
  return new Date(epochSecs * 1000).toLocaleTimeString();
}

/** What a "host" rule should match for this connection. */
export function hostRuleFor(r: ConnDto): {
  kind: RuleType;
  value: string;
} {
  const h = hostOf(r);
  return isIpv4(h)
    ? { kind: "IP-CIDR", value: `${h}/32` }
    : { kind: "DOMAIN", value: h };
}
