import { readFileSync } from "node:fs";
import { test, expect } from "vitest";

const source = readFileSync(
  new URL("./network-section.svelte", import.meta.url),
  "utf8",
);

test("GeoIP copy does not conflate country lookups with routing rule sets", () => {
  expect(source).not.toContain("Country database used by GeoIP routing rules.");
  expect(source).toContain("IP details and flags");
  expect(source).toContain("Routing rules use separate rule sets.");
});
