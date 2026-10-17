import { describe, expect, test } from "vitest";
import type { DnsLookupResult } from "./api";
import { compareDnsAnswers, isFakeIp, lookupRecords, normalizeLookupDomain } from "./dns-diagnostics";

function result(ips: string[], status: number | null = 0): DnsLookupResult {
  return {
    resolved_ips: ips, status, latency_ms: 1, server_used: "internal",
    record_type: "A", ttl: null, records: [], trace: null,
  };
}

describe("DNS comparisons", () => {
  test("address order and duplicates do not create a false mismatch", () => {
    expect(compareDnsAnswers(result(["1.1.1.1", "8.8.8.8", "1.1.1.1"]), result(["8.8.8.8", "1.1.1.1"], null))).toBe("same");
  });
  test("actual FakeIP responses explain a difference independently of saved configuration", () => {
    expect(compareDnsAnswers(result(["198.19.255.255"]), result(["1.1.1.1"]))).toBe("fakeip");
  });
  test("different public answers are described neutrally", () => {
    expect(compareDnsAnswers(result(["1.1.1.1"]), result(["8.8.8.8"]))).toBe("different");
  });
  test("failed or empty responses cannot be called equivalent or mismatched", () => {
    expect(compareDnsAnswers(result([], 3), result(["8.8.8.8"]))).toBe("inconclusive");
    expect(compareDnsAnswers(result([]), result([]))).toBe("inconclusive");
  });
});

test.each([
  ["198.18.0.0", true], ["198.19.255.255", true], ["198.17.255.255", false],
  ["198.20.0.0", false], ["198.18.0.999", false], ["::1", false],
])("FakeIP range classification of %s", (ip, expected) => {
  expect(isFakeIp(ip as string)).toBe(expected);
});

test("complete DNS records retain sections, TTLs, and non-address data", () => {
  const response = result([]);
  response.records = [{ name: "example.com.", record_type: "CNAME", data: "edge.example.com.", ttl: 42, section: "answer" }];
  expect(lookupRecords(response)).toEqual(response.records);
});

test("legacy address responses retain an answer fallback without invented TTLs", () => {
  expect(lookupRecords(result(["::1"]))).toEqual([{ name: "", data: "::1", record_type: "AAAA", ttl: null, section: "answer" }]);
});

test("lookup domain validation accepts DNS names and rejects URLs, ports, and empty labels", () => {
  expect(normalizeLookupDomain("  Example.COM. ")).toBe("example.com");
  expect(normalizeLookupDomain("https://example.com/path")).toBeNull();
  expect(normalizeLookupDomain("example.com:443")).toBeNull();
  expect(normalizeLookupDomain("example..com")).toBeNull();
});
