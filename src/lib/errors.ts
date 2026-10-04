/**
 * Turning thrown values into text a user can act on.
 *
 * Every Tauri command in this app fails with `CommandError`, which is
 * `#[serde(tag = "kind", content = "message")]` — so what actually arrives is a
 * plain object `{ kind: "Internal", message: "…" }`, never an `Error`. That is
 * why `String(err)` printed `[object Object]` and `err instanceof Error` was
 * always false. The Display text from `thiserror` (`"Core error: …"`) never
 * crosses the bridge; serde does not use it.
 */

/** Categories `CommandError` can carry. Kept for callers that branch on them. */
export type ErrorKind =
  | "Core"
  | "Api"
  | "State"
  | "Network"
  | "NotFound"
  | "InvalidInput"
  | "Internal";

const FALLBACK = "Something went wrong.";

/**
 * The most specific human-readable message available in `err`.
 *
 * Never returns `[object Object]`, an empty string, or `"undefined"`: an
 * unrecognised shape is JSON-encoded so the detail is at least visible in a
 * bug report. Pass `fallback` to control what a value carrying no message
 * produces.
 */
export function errorMessage(err: unknown, fallback: string = FALLBACK): string {
  const text = extract(err);
  return text ?? fallback;
}

/**
 * The `CommandError` category, when the value is one.
 *
 * Useful for branching — a `NotFound` usually deserves different copy than a
 * `Network` failure.
 */
export function errorKind(err: unknown): ErrorKind | null {
  if (!isRecord(err)) return null;
  const kind = err.kind;
  return typeof kind === "string" && KINDS.has(kind) ? (kind as ErrorKind) : null;
}

const KINDS: ReadonlySet<string> = new Set([
  "Core",
  "Api",
  "State",
  "Network",
  "NotFound",
  "InvalidInput",
  "Internal",
]);

function extract(err: unknown): string | null {
  if (err === null || err === undefined) return null;

  // Commands that fail with a bare String, and anything re-thrown as text.
  if (typeof err === "string") return clean(err);

  if (err instanceof Error) return clean(err.message);

  if (isRecord(err)) {
    // CommandError and any `{ message }` shape. `content` is serde's name when
    // a variant is renamed, so accept it too.
    let sawKnownKey = false;
    for (const key of ["message", "content", "error", "reason"] as const) {
      const value = err[key];
      if (typeof value === "string") {
        sawKnownKey = true;
        const text = clean(value);
        if (text) return text;
      }
      // A nested error — one level is enough to reach the real cause.
      if (isRecord(value) || value instanceof Error) {
        sawKnownKey = true;
        const nested = extract(value);
        if (nested) return nested;
      }
    }
    // A shape we recognise but whose message is blank: the caller's fallback
    // reads better than dumping `{"message":"  "}` at the user.
    if (sawKnownKey) return null;
    // Unrecognised object: show the payload rather than "[object Object]".
    // Always return here — `String()` throws on a null-prototype object.
    return safeJson(err);
  }

  // Numbers, booleans, symbols — rare, but printable.
  const coerced = clean(String(err));
  return coerced === "[object Object]" ? null : coerced;
}

function clean(value: string): string | null {
  const trimmed = value.trim();
  return trimmed.length > 0 ? trimmed : null;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function safeJson(value: unknown): string | null {
  try {
    const encoded = JSON.stringify(value);
    return encoded && encoded !== "{}" ? encoded : null;
  } catch {
    // Cyclic or otherwise unserialisable.
    return null;
  }
}
