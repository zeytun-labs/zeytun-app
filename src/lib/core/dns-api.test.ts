import { beforeEach, expect, test, vi } from "vitest";

const invoke = vi.hoisted(() => vi.fn());
vi.mock("@tauri-apps/api/core", () => ({ invoke }));
import { testDnsLookup, testSystemDnsLookup } from "./api";

beforeEach(() => invoke.mockReset());

test("ordinary Core lookups retain automatic routing with explicit defaults", async () => {
  await testDnsLookup("example.com");
  expect(invoke).toHaveBeenCalledWith("test_dns_lookup", {
    domain: "example.com", recordType: "A", server: null, bypassCache: false,
  });
});

test("advanced Core lookups use the native camelCase argument contract", async () => {
  await testDnsLookup("example.com", "AAAA", "local-dns", true);
  expect(invoke).toHaveBeenCalledWith("test_dns_lookup", {
    domain: "example.com", recordType: "AAAA", server: "local-dns", bypassCache: true,
  });
});

test("System lookup uses its separate command without Core-only options", async () => {
  await testSystemDnsLookup("example.com", "AAAA");
  expect(invoke).toHaveBeenCalledWith("test_system_dns_lookup", {
    domain: "example.com", recordType: "AAAA",
  });
});
