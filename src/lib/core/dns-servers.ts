import { z } from "zod";
import type { DnsConfig, DnsServer } from "./types";
import { FOLLOW_MODE_DETOUR } from "./dns-policy";

export const DNS_PROTOCOLS = ["DoH", "DoT", "TCP", "UDP", "QUIC"] as const;
export type DnsProtocol = (typeof DNS_PROTOCOLS)[number];
export const LOCAL_DNS_SERVER_TAG = "local-dns";

export function dnsDetourExists(
  detour: string | null | undefined,
  outbounds: { policies: { tag: string }[]; proxies: { tag: string }[] },
): boolean {
  return !detour || detour === FOLLOW_MODE_DETOUR || detour.toLowerCase() === "direct" ||
    outbounds.policies.some((policy) => policy.tag === detour) ||
    outbounds.proxies.some((proxy) => proxy.tag === detour);
}

export function dnsRuleTargetExists(target: string, servers: DnsServer[]): boolean {
  return target === "block" || target === LOCAL_DNS_SERVER_TAG ||
    servers.some((server) => server.tag === target);
}

export function dnsRuleTargetLabel(target: string, servers: DnsServer[]): string {
  if (target === "block") return "Block";
  if (target === LOCAL_DNS_SERVER_TAG) return "Local (System)";
  return servers.find((server) => server.tag === target)?.name || `${target} (missing DNS server)`;
}

const prefixes: Record<DnsProtocol, string> = {
  DoH: "https://",
  DoT: "tls://",
  TCP: "tcp://",
  UDP: "",
  QUIC: "quic://",
};

export function dnsProtocolFromAddress(address: string): DnsProtocol {
  const scheme = address.trim().split("://")[0].toLowerCase();
  switch (scheme) {
    case "https": return "DoH";
    case "tls": return "DoT";
    case "tcp": return "TCP";
    case "quic": return "QUIC";
    default: return "UDP";
  }
}

export function dnsAddressWithoutProtocol(address: string): string {
  return address.replace(/^[a-z][a-z0-9+.-]*:\/\//i, "");
}

/** Validate the exact upstream endpoint, rather than silently changing protocols. */
export function normalizeDnsServerAddress(
  raw: string,
  protocol: DnsProtocol,
): string {
  const address = raw.trim();
  if (!address) throw new Error("Server address is required.");
  if (/[\s\u0000-\u001f\u007f]/.test(address)) {
    throw new Error("Server addresses cannot contain spaces or control characters.");
  }
  if (address.includes("\\")) {
    throw new Error("DNS server addresses cannot contain backslashes.");
  }

  const scheme = address.match(/^([a-z][a-z0-9+.-]*):\/\//i)?.[1]?.toLowerCase();
  if (scheme && !["https", "tls", "tcp", "udp", "quic"].includes(scheme)) {
    throw new Error("Use HTTPS, TLS, TCP, UDP, or QUIC for a DNS server.");
  }
  if (scheme && dnsProtocolFromAddress(address) !== protocol) {
    throw new Error("The address protocol does not match the selected protocol.");
  }

  const endpoint = dnsAddressWithoutProtocol(address);
  const authority = endpoint.split(/[/?#]/, 1)[0];
  const suffix = endpoint.slice(authority.length);
  if (endpoint.includes("@") || endpoint.includes("#")) {
    throw new Error("DNS server addresses cannot contain credentials or fragments.");
  }
  if (protocol !== "DoH" && suffix !== "") {
    throw new Error("Only DNS over HTTPS supports a path or query. Enter only a hostname or IP address and optional port.");
  }

  let host: string;
  let explicitPort: string | undefined;
  if (authority.startsWith("[")) {
    const bracketed = authority.match(/^\[([^\]]+)\](?::(.*))?$/);
    if (!bracketed || !z.ipv6().safeParse(bracketed[1]).success) {
      throw new Error("Enter a valid IPv6 address in brackets, with an optional port.");
    }
    host = bracketed[1];
    explicitPort = bracketed[2];
  } else if (z.ipv6().safeParse(authority).success) {
    host = authority;
  } else {
    const split = authority.lastIndexOf(":");
    host = split === -1 ? authority : authority.slice(0, split);
    explicitPort = split === -1 ? undefined : authority.slice(split + 1);
  }
  host = host.replace(/\.+$/, "").toLowerCase();
  const ipv4 = z.ipv4().safeParse(host).success;
  const ipv6 = z.ipv6().safeParse(host).success;
  const isDomain = host.length <= 253 && !/^[\d.]+$/.test(host) &&
    host.split(".").every(
      (label) => label.length <= 63 && /^[a-z0-9](?:[a-z0-9-]*[a-z0-9])?$/i.test(label),
    );
  if (!ipv4 && !ipv6 && !isDomain) {
    throw new Error("Enter a valid DNS server hostname or IP address.");
  }
  if (ipv6) host = new URL(`https://[${host}]`).hostname.slice(1, -1);
  if (
    host === "0.0.0.0" || host === "::" ||
    (ipv4 && Number(host.split(".")[0]) >= 224 && Number(host.split(".")[0]) <= 239) ||
    (ipv6 && host.startsWith("ff"))
  ) {
    throw new Error("DNS servers must use a unicast address, not an unspecified or multicast address.");
  }
  if (explicitPort !== undefined && (
    !/^\d+$/.test(explicitPort) || Number(explicitPort) < 1 || Number(explicitPort) > 65535
  )) {
    throw new Error("Enter a port between 1 and 65535.");
  }

  const hostAndPort = `${ipv6 ? `[${host}]` : host}${explicitPort ? `:${Number(explicitPort)}` : ""}`;
  const path = protocol === "DoH"
    ? suffix === "" || suffix.startsWith("?") ? `/dns-query${suffix}` : suffix
    : "";
  return `${prefixes[protocol]}${hostAndPort}${path}`;
}

export type DnsServerDraft = {
  name: string;
  address: string;
  protocol: DnsProtocol;
  detour: string | null;
};

/** An edit keeps the tag and array position, so existing rules keep their target. */
export function upsertDnsServer(
  servers: DnsServer[],
  draft: DnsServerDraft,
  tag: string,
  editing: boolean,
): DnsServer[] {
  const name = draft.name.trim();
  if (!name) throw new Error("Server name is required.");
  const address = normalizeDnsServerAddress(draft.address, draft.protocol);
  const index = servers.findIndex((server) => server.tag === tag);
  if (editing && index === -1) {
    throw new Error("This DNS server no longer exists. Close the dialog and try again.");
  }
  if (!editing && index !== -1) {
    throw new Error("A DNS server with this identifier already exists.");
  }
  const server: DnsServer = {
    ...(editing ? servers[index] : {}),
    tag,
    name,
    address,
    detour: draft.detour || (editing ? undefined : FOLLOW_MODE_DETOUR),
  };
  return editing
    ? servers.map((current) => current.tag === tag ? server : current)
    : [...servers, server];
}

export function dnsServerDeleteError(dns: DnsConfig, tag: string): string | null {
  const references = (dns.rules ?? []).filter((rule) => rule.target === tag).length;
  const uses: string[] = [];
  if (dns.final_server === tag) uses.push("the final DNS resolver");
  if (references > 0) uses.push(`${references} DNS rule${references === 1 ? "" : "s"} (including disabled rules)`);
  return uses.length > 0
    ? `This server is used by ${uses.join(" and ")}. Choose another resolver for these references before deleting it.`
    : null;
}
