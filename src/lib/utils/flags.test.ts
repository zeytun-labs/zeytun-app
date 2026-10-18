import { describe, it, expect, vi } from "vitest";
import { initFlagPolyfill } from "./flags";

describe("flags utility", () => {
  it("should export initFlagPolyfill function", () => {
    expect(typeof initFlagPolyfill).toBe("function");
  });

  it("should execute safely without errors", () => {
    expect(() => initFlagPolyfill()).not.toThrow();
  });
});
