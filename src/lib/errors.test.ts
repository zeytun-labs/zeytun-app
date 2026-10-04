/**
 * Runnable check for `errorMessage`.
 *
 *   pnpm test
 *
 * The cases that matter are the shapes the Tauri bridge actually delivers.
 */
import { describe, it, expect } from "vitest";
import { errorMessage, errorKind } from "./errors";

describe("errorMessage", () => {
  it("CommandError from the Tauri bridge yields its message", () => {
    // What `#[serde(tag = "kind", content = "message")]` actually sends.
    const err = { kind: "Internal", message: "cannot write /tmp/x: permission denied" };
    expect(errorMessage(err)).toBe("cannot write /tmp/x: permission denied");
    expect(errorKind(err)).toBe("Internal");
  });

  it("never returns [object Object]", () => {
    // The bug this module exists to kill.
    for (const value of [{}, { foo: 1 }, { kind: "Api" }, Object.create(null)]) {
      const text = errorMessage(value);
      expect(text).not.toBe("[object Object]");
      expect(text.length).toBeGreaterThan(0);
    }
  });

  it("an unrecognised object shows its payload rather than a useless string", () => {
    expect(errorMessage({ foo: 1 })).toBe('{"foo":1}');
  });

  it("strings, Errors, and empty values", () => {
    expect(errorMessage("plain failure")).toBe("plain failure");
    expect(errorMessage(new Error("boom"))).toBe("boom");
    expect(errorMessage("  padded  ")).toBe("padded");
    // Nothing usable → the caller's fallback, never "undefined" or "null".
    expect(errorMessage(undefined)).toBe("Something went wrong.");
    expect(errorMessage(null)).toBe("Something went wrong.");
    expect(errorMessage("")).toBe("Something went wrong.");
    expect(errorMessage({ message: "   " }, "custom")).toBe("custom");
  });

  it("nested causes are reached one level down", () => {
    expect(errorMessage({ error: { message: "inner detail" } })).toBe("inner detail");
  });

  it("cyclic objects do not throw", () => {
    const cyclic: Record<string, unknown> = { kind: "Core" };
    cyclic.self = cyclic;
    expect(() => errorMessage(cyclic)).not.toThrow();
    expect(errorKind(cyclic)).toBe("Core");
  });
});

describe("errorKind", () => {
  it("rejects anything that is not a CommandError variant", () => {
    expect(errorKind({ kind: "Bogus" })).toBeNull();
    expect(errorKind(new Error("x"))).toBeNull();
    expect(errorKind("Internal")).toBeNull();
  });
});
