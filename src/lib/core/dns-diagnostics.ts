import type { DnsLookupResult, DnsRecord } from "./api";

export function normalizeLookupDomain(value: string): string | null {
  const domain = value.trim().toLowerCase().replace(/\.$/, "");
  return domain.length > 0 && domain.length <= 253 && domain.split(".").every(
    (label) => label.length <= 63 && /^[a-z0-9](?:[a-z0-9-]*[a-z0-9])?$/.test(label),
  ) ? domain : null;
}

/** Classify returned addresses rather than guessing from the saved FakeIP setting. */
export function isFakeIp(ip: string): boolean {
  const parts = ip.split(".");
  return parts.length === 4 && parts.every((part) =>
    /^\d{1,3}$/.test(part) && Number(part) <= 255,
  ) && parts[0] === "198" && (parts[1] === "18" || parts[1] === "19");
}

export function lookupRecords(result: DnsLookupResult): DnsRecord[] {
  if (result.records?.length) return result.records;
  // Keep ordinary lookups useful when communicating with an older app backend.
  return result.resolved_ips.map((ip) => ({
    name: "",
    record_type: ip.includes(":") ? "AAAA" : "A",
    data: ip,
    ttl: result.ttl,
    section: "answer",
  }));
}

export type DnsComparison = "same" | "fakeip" | "different" | "inconclusive";

export function compareDnsAnswers(core: DnsLookupResult, system: DnsLookupResult): DnsComparison {
  if (
    (core.status !== null && core.status !== 0) ||
    (system.status !== null && system.status !== 0) ||
    core.resolved_ips.length === 0 || system.resolved_ips.length === 0
  ) return "inconclusive";

  const coreAddresses = new Set(core.resolved_ips);
  const systemAddresses = new Set(system.resolved_ips);
  if (coreAddresses.size === systemAddresses.size &&
    [...coreAddresses].every((ip) => systemAddresses.has(ip))) return "same";
  return core.resolved_ips.some(isFakeIp) || system.resolved_ips.some(isFakeIp)
    ? "fakeip"
    : "different";
}
