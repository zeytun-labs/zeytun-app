import type { DnsConfig, DnsServer, OutboundMode } from "./types";

export const FOLLOW_MODE_DETOUR = "follow-mode";
export const DNS_DIRECT_TAG_PREFIX = "__dns-direct-";

type NamedDnsServer = Pick<DnsServer, "tag" | "name">;

/** Explicit lookups must select the same half that connection-mode routing uses. */
export function dnsLookupServerTag(
  selectedTag: string | null,
  servers: readonly Pick<DnsServer, "tag" | "detour">[],
  outbound: OutboundMode,
): string | null {
  if (outbound === "direct" && selectedTag &&
    servers.some((server) => server.tag === selectedTag && server.detour === FOLLOW_MODE_DETOUR)) {
    return `${DNS_DIRECT_TAG_PREFIX}${selectedTag}`;
  }
  return selectedTag;
}

/** Runtime IDs remain available in Technical details, not the primary label. */
export function dnsResolverLabel(
  tag: string | null | undefined,
  servers: readonly NamedDnsServer[] = [],
): string {
  const id = tag?.trim();
  if (!id || id.toLowerCase() === "internal") return "Core DNS";
  if (id === "local-dns") return "Local (System)";
  if (id === "fakeip") return "FakeIP";
  if (id === "local-hosts") return "Local Hosts";
  const configured = servers.find((server) => server.tag === id)?.name.trim();
  if (configured) return configured;
  if (id.startsWith(DNS_DIRECT_TAG_PREFIX)) {
    const original = servers.find((server) => server.tag === id.slice(DNS_DIRECT_TAG_PREFIX.length))?.name.trim();
    return original ? `${original} · Direct` : "Direct DNS";
  }
  return "DNS resolver";
}

function finalDnsServer(dns: DnsConfig | null | undefined): DnsServer | undefined {
  return dns?.servers?.find((server) => server.tag === dns.final_server?.trim());
}

function usesDirectDns(dns: DnsConfig | null | undefined, target: string | null | undefined): boolean {
  const tag = target?.trim();
  if (!tag || tag === "local-dns") return true;
  const server = dns?.servers?.find((server) => server.tag === tag);
  // Legacy missing detours retain Direct; only the explicit sentinel follows mode.
  return server !== undefined && (!server.detour?.trim() || server.detour.trim().toLowerCase() === "direct");
}

export function dnsPolicySummary(dns: DnsConfig | null | undefined, outbound: OutboundMode): string {
  const tag = dns?.final_server?.trim() || "local-dns";
  if (tag === "local-dns") return "Unmatched queries use Local (System).";
  const server = finalDnsServer(dns);
  if (!server) return "Choose an available Final resolver in DNS Rules.";
  if (server.detour === FOLLOW_MODE_DETOUR) {
    return `Unmatched queries use ${server.name} ${outbound === "direct" ? "directly" : "through your selected proxy"}.`;
  }
  return usesDirectDns(dns, tag)
    ? `Unmatched queries use ${server.name} directly.`
    : `Unmatched queries use ${server.name} through its configured outbound.`;
}

export function dnsPolicyDescription(dns: DnsConfig | null | undefined, outbound: OutboundMode): string {
  return `${dnsPolicySummary(dns, outbound)} Hosts and specific DNS rules take priority over Final. Local names stay local.${dns?.fake_ip && outbound !== "direct" ? " Unmatched A / AAAA queries use FakeIP before Final." : ""}`;
}

export function dnsPolicyWarnings(
  dns: DnsConfig | null | undefined,
  outbound: OutboundMode,
  selectedProxyMember?: string | null,
): string[] {
  if (outbound === "direct") return [];
  const warnings: string[] = [];
  if (outbound === "global" && usesDirectDns(dns, dns?.final_server)) {
    warnings.push("Final DNS uses Direct or Local (System), including in Global mode.");
  }
  const directRules = dns?.rules?.filter((rule) => rule.enabled && usesDirectDns(dns, rule.target)).length ?? 0;
  if (directRules > 0) {
    warnings.push(`${directRules} enabled DNS rule${directRules === 1 ? " explicitly uses" : "s explicitly use"} Direct or Local (System).`);
  }
  const followsMode = finalDnsServer(dns)?.detour === FOLLOW_MODE_DETOUR || dns?.rules?.some((rule) =>
    rule.enabled && dns.servers?.some((server) => server.tag === rule.target && server.detour === FOLLOW_MODE_DETOUR),
  );
  if (followsMode && selectedProxyMember?.trim().toLowerCase() === "direct") {
    warnings.push("The selected proxy explicitly uses Direct. DNS servers that follow connection mode use this explicit choice.");
  }
  return warnings;
}

/** Describe the saved configuration, without implying it is already applied. */
export function dnsResolutionDetails(
  dns: DnsConfig | null | undefined,
  outbound: OutboundMode,
  outboundNames: Readonly<Record<string, string>> = {},
) {
  const tag = dns?.final_server?.trim() || "local-dns";
  const server = finalDnsServer(dns);
  const resolver = tag === "local-dns" ? "Local (System)" : server?.name || "Unavailable DNS server";
  const mode = outbound === "global" ? "Global" : outbound === "direct" ? "Direct" : "Rule";
  const detour = server?.detour?.trim();
  const route = tag === "local-dns" ? "Physical network DNS"
    : !server ? "Choose an available Final resolver in DNS Rules"
    : detour === FOLLOW_MODE_DETOUR ? `Follow connection mode · ${outbound === "direct" ? "Direct" : `${mode} → Selected proxy`}`
    : !detour || detour.toLowerCase() === "direct" ? "Direct · fixed route"
    : `Fixed outbound · ${outboundNames[detour] || "Configured policy or proxy"}`;
  const fakeIpEffective = !!dns?.fake_ip && outbound !== "direct";
  return {
    resolver,
    address: server?.address ?? null,
    route,
    steps: ["Hosts", "DNS rules", "Local names", ...(fakeIpEffective ? ["FakeIP for A / AAAA"] : []), `Final (${resolver})`],
    fakeIpBypassed: !!dns?.fake_ip && outbound === "direct",
  };
}
