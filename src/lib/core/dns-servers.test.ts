import { describe, expect, test } from "vitest";
import {
  dnsProtocolFromAddress,
  dnsDetourExists,
  dnsRuleTargetExists,
  dnsRuleTargetLabel,
  dnsServerDeleteError,
  normalizeDnsServerAddress,
  upsertDnsServer,
  type DnsProtocol,
} from "./dns-servers";
import type { DnsConfig, DnsServer } from "./types";

describe("DNS endpoint validation", () => {
  test.each<[string, DnsProtocol, string]>([
    ["  DNS.GOOGLE  ", "DoH", "https://dns.google/dns-query"],
    ["HTTPS://dns.google:8443/custom?token=abc", "DoH", "https://dns.google:8443/custom?token=abc"],
    ["https://dns.google:443/dns-query", "DoH", "https://dns.google:443/dns-query"],
    ["dns.google:443", "DoT", "tls://dns.google:443"],
    ["tls://dns.google:853", "DoT", "tls://dns.google:853"],
    ["1.1.1.1:5353", "UDP", "1.1.1.1:5353"],
    ["udp://1.1.1.1", "UDP", "1.1.1.1"],
    ["2001:4860:4860::8888", "UDP", "[2001:4860:4860::8888]"],
    ["[2606:4700:4700::1111]:5353", "TCP", "tcp://[2606:4700:4700::1111]:5353"],
    ["quic://dns.adguard-dns.com:853", "QUIC", "quic://dns.adguard-dns.com:853"],
    ["https://2001:4860:4860::8888/query?key=abc", "DoH", "https://[2001:4860:4860::8888]/query?key=abc"],
    ["https://dns.google/", "DoH", "https://dns.google/"],
    ["https://dns.google?key=abc", "DoH", "https://dns.google/dns-query?key=abc"],
  ])("normalizes %s using %s", (address, protocol, expected) => {
    expect(normalizeDnsServerAddress(address, protocol)).toBe(expected);
  });

  test.each<[string, DnsProtocol]>([
    ["", "DoH"],
    ["dns. google", "DoH"],
    ["http://dns.google", "DoH"],
    ["https://dns.google/dns-query", "UDP"],
    ["https://user:password@dns.google/dns-query", "DoH"],
    ["https://dns.google/dns-query#fragment", "DoH"],
    ["1.1.1.1:0", "UDP"],
    ["1.1.1.1:65536", "UDP"],
    ["1.1.1.1:", "TCP"],
    ["1.1.1.999", "UDP"],
    ["1.1.1.1/dns-query", "UDP"],
    ["dns.google?token=abc", "DoT"],
    ["-invalid.example", "DoH"],
    ["dns.google\\dns-query", "DoH"],
    ["127.1", "UDP"],
    ["0.0.0.0", "UDP"],
    ["::", "UDP"],
    ["[0:0:0:0:0:0:0:0]:53", "TCP"],
    ["224.0.0.1", "UDP"],
    ["239.255.255.255:53", "TCP"],
    ["ff02::1", "UDP"],
    ["https://[ff01::1]/dns-query", "DoH"],
  ])("rejects invalid endpoint %s using %s", (address, protocol) => {
    expect(() => normalizeDnsServerAddress(address, protocol)).toThrow();
  });

  test("recognizes pasted protocol names case insensitively", () => {
    expect(dnsProtocolFromAddress(" HTTPS://dns.google/dns-query ")).toBe("DoH");
    expect(dnsProtocolFromAddress("tls://dns.google")).toBe("DoT");
    expect(dnsProtocolFromAddress("udp://1.1.1.1")).toBe("UDP");
  });
});

describe("DNS server edits and references", () => {
  const servers: DnsServer[] = [
    { tag: "google", name: "Google", address: "https://dns.google/dns-query", detour: "root-policy" },
    { tag: "cloudflare", name: "Cloudflare", address: "1.1.1.1" },
  ];
  const dns: DnsConfig = {
    servers,
    final_server: "google",
    rules: [{ id: "discord", kind: "domain_suffix", value: "discord.com", target: "google", enabled: false }],
  };

  test("editing preserves server tag, list order, unrelated servers and references", () => {
    const result = upsertDnsServer(servers, {
      name: " Updated Google ", address: "8.8.8.8", protocol: "DoT", detour: "root-policy",
    }, "google", true);
    expect(result).toEqual([
      { tag: "google", name: "Updated Google", address: "tls://8.8.8.8", detour: "root-policy" },
      servers[1],
    ]);
    expect(result[1]).toBe(servers[1]);
    expect(servers[0].name).toBe("Google");
    expect(dns.final_server).toBe(result[0].tag);
    expect(dns.rules?.[0].target).toBe(result[0].tag);
  });

  test("clearing an existing detour retains legacy Direct semantics", () => {
    const result = upsertDnsServer(servers, {
      name: "Google", address: "dns.google", protocol: "DoH", detour: null,
    }, "google", true);
    expect(result[0].detour).toBeUndefined();
  });

  test("new DNS servers follow connection mode while editing legacy Direct keeps its route", () => {
    const draft = { name: "New", address: "1.1.1.1", protocol: "UDP" as const, detour: null };
    expect(upsertDnsServer(servers, draft, "new", false).at(-1)?.detour).toBe("follow-mode");
    expect(upsertDnsServer(servers, draft, "cloudflare", true).find((server) => server.tag === "cloudflare")?.detour).toBeUndefined();
    expect(upsertDnsServer(servers, { ...draft, detour: "direct" }, "new", false).at(-1)?.detour).toBe("direct");
  });

  test("an unavailable editing target cannot silently turn into a new server", () => {
    expect(() => upsertDnsServer(servers, {
      name: "Removed server", address: "1.1.1.1", protocol: "UDP", detour: null,
    }, "missing", true)).toThrow("no longer exists");
  });

  test("deletion protects final and disabled rule references", () => {
    expect(dnsServerDeleteError(dns, "google")).toContain("final DNS resolver");
    expect(dnsServerDeleteError(dns, "google")).toContain("1 DNS rule");
    expect(dnsServerDeleteError(dns, "cloudflare")).toBeNull();
  });

  test("built-in system DNS is available without custom servers and has a readable label", () => {
    expect(dnsRuleTargetExists("local-dns", [])).toBe(true);
    expect(dnsRuleTargetLabel("local-dns", [])).toBe("Local (System)");
    expect(dnsRuleTargetExists("block", [])).toBe(true);
    expect(dnsRuleTargetExists("missing", servers)).toBe(false);
    expect(dnsRuleTargetExists("google", servers)).toBe(true);
    expect(dnsRuleTargetLabel("google", servers)).toBe("Google");
  });

  test("DNS detours accept individual proxies as well as policies and direct", () => {
    const outbounds = {
      policies: [{ tag: "root-policy" }],
      proxies: [{ tag: "imported-proxy" }],
    };
    expect(dnsDetourExists("imported-proxy", outbounds)).toBe(true);
    expect(dnsDetourExists("root-policy", outbounds)).toBe(true);
    expect(dnsDetourExists("follow-mode", outbounds)).toBe(true);
    expect(dnsDetourExists("DIRECT", outbounds)).toBe(true);
    expect(dnsDetourExists(null, outbounds)).toBe(true);
    expect(dnsDetourExists("deleted-proxy", outbounds)).toBe(false);
  });
});
