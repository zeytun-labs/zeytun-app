import { expect, test } from "vitest";
import type { DnsConfig, DnsServer } from "./types";
import { dnsResolutionDetails, dnsLookupServerTag, dnsPolicyWarnings, dnsResolverLabel, dnsPolicySummary } from "./dns-policy";

const servers: DnsServer[] = [
  { tag: "google", name: "Google", address: "https://8.8.8.8/dns-query", detour: "follow-mode" },
  { tag: "local-net", name: "Local network", address: "1.1.1.1", detour: "direct" },
  { tag: "legacy", name: "Legacy", address: "9.9.9.9" },
  { tag: "fixed-proxy", name: "Fixed proxy", address: "8.8.4.4", detour: "my-proxy" },
];

test("follow-mode explicit lookups choose the Direct half only in Direct mode", () => {
  expect(dnsLookupServerTag("google", servers, "direct")).toBe("__dns-direct-google");
  expect(dnsLookupServerTag("google", servers, "global")).toBe("google");
  expect(dnsLookupServerTag("google", servers, "rule")).toBe("google");
  expect(dnsLookupServerTag("fixed-proxy", servers, "direct")).toBe("fixed-proxy");
  expect(dnsLookupServerTag("local-net", servers, "direct")).toBe("local-net");
  expect(dnsLookupServerTag("legacy", servers, "direct")).toBe("legacy");
  expect(dnsLookupServerTag(null, servers, "direct")).toBeNull();
  expect(dnsLookupServerTag("local-dns", servers, "direct")).toBe("local-dns");
});

test("runtime resolver IDs get readable names, including generated Direct halves", () => {
  expect(dnsResolverLabel("google", servers)).toBe("Google");
  expect(dnsResolverLabel("__dns-direct-google", servers)).toBe("Google · Direct");
  expect(dnsResolverLabel("__dns-direct-removed", servers)).toBe("Direct DNS");
  expect(dnsResolverLabel("local-dns")).toBe("Local (System)");
  expect(dnsResolverLabel("fakeip")).toBe("FakeIP");
  expect(dnsResolverLabel("local-hosts")).toBe("Local Hosts");
  expect(dnsResolverLabel("removed-internal-id")).toBe("DNS resolver");
  expect(dnsResolverLabel("internal")).toBe("Core DNS");
  expect(dnsResolverLabel(null)).toBe("Core DNS");
});

test("Final summaries distinguish follow-mode from explicit and legacy outbounds", () => {
  expect(dnsPolicySummary({ servers, final_server: "google" }, "global")).toContain("selected proxy");
  expect(dnsPolicySummary({ servers, final_server: "google" }, "direct")).toContain("directly");
  expect(dnsPolicySummary({ servers, final_server: "legacy" }, "global")).toContain("directly");
  expect(dnsPolicySummary({ servers, final_server: "fixed-proxy" }, "direct")).toContain("configured outbound");
  expect(dnsPolicySummary({ final_server: null }, "global")).toContain("Local (System)");
});

test("Global warns for Direct/Local Final without treating follow-mode as Direct", () => {
  expect(dnsPolicyWarnings({ servers, final_server: "google" }, "global")).toEqual([]);
  expect(dnsPolicyWarnings({ servers, final_server: "local-net" }, "global")).toHaveLength(1);
  expect(dnsPolicyWarnings({ servers, final_server: "legacy" }, "global")).toHaveLength(1);
  expect(dnsPolicyWarnings({ servers, final_server: null }, "global")).toHaveLength(1);
  expect(dnsPolicyWarnings({ servers, final_server: "missing" }, "global")).toEqual([]);
  expect(dnsPolicyWarnings({ servers, final_server: "fixed-proxy" }, "global")).toEqual([]);
});

test("only enabled Direct/Local rules and an explicitly Direct selected proxy produce notices", () => {
  const dns: DnsConfig = {
    servers, final_server: "google", rules: [
      { id: "local", kind: "domain", value: "local.test", target: "local-dns", enabled: true },
      { id: "legacy", kind: "domain", value: "legacy.test", target: "legacy", enabled: true },
      { id: "off", kind: "domain", value: "off.test", target: "local-net", enabled: false },
      { id: "proxy", kind: "domain", value: "proxy.test", target: "fixed-proxy", enabled: true },
      { id: "block", kind: "domain", value: "block.test", target: "block", enabled: true },
    ],
  };
  expect(dnsPolicyWarnings(dns, "global")).toEqual([expect.stringContaining("2 enabled DNS rules")]);
  expect(dnsPolicyWarnings(dns, "direct")).toEqual([]);
  expect(dnsPolicyWarnings(dns, "rule", "DIRECT")).toEqual([
    expect.stringContaining("2 enabled DNS rules"), expect.stringContaining("explicitly uses Direct"),
  ]);
});


test("resolution details follow Final and its configured route rather than a fixed provider", () => {
  const first = dnsResolutionDetails({ servers, final_server: "google" }, "global");
  const next = dnsResolutionDetails({ servers, final_server: "fixed-proxy" }, "global", { "my-proxy": "My proxy" });
  expect(first.resolver).toBe("Google");
  expect(first.route).toBe("Follow connection mode · Global → Selected proxy");
  expect(next.resolver).toBe("Fixed proxy");
  expect(next.address).toBe("8.8.4.4");
  expect(next.route).toBe("Fixed outbound · My proxy");
  expect(next.steps.at(-1)).toBe("Final (Fixed proxy)");
  expect(dnsResolutionDetails({ servers, final_server: "fixed-proxy" }, "direct", { "my-proxy": "My proxy" }).route).toBe(next.route);
});

test("resolution details show current mode, FakeIP bypass, and Local without claiming another endpoint", () => {
  const dns = { servers, final_server: "google", fake_ip: true };
  const direct = dnsResolutionDetails(dns, "direct");
  expect(direct.route).toBe("Follow connection mode · Direct");
  expect(direct.fakeIpBypassed).toBe(true);
  expect(direct.steps).not.toContain("FakeIP for A / AAAA");
  expect(dnsResolutionDetails(dns, "rule").steps).toContain("FakeIP for A / AAAA");
  const local = dnsResolutionDetails({ servers, final_server: null }, "global");
  expect(local.resolver).toBe("Local (System)");
  expect(local.route).toBe("Physical network DNS");
  expect(local.address).toBeNull();
  const missing = dnsResolutionDetails({ servers, final_server: "missing" }, "global");
  expect(missing.resolver).toBe("Unavailable DNS server");
  expect(missing.address).toBeNull();
});
