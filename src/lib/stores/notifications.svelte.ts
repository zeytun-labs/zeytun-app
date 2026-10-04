import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { toast } from "svelte-sonner";

export type CoreNotification = {
  id: string;
  ts_ms: number;
  scope: number;
  code: string;
  severity: number; // 0 info 1 warn 2 error 3 critical
  title: string;
  message: string;
  attrs: Record<string, string>;
  read: boolean;
  /** User dismissed a sticky critical banner (still in history). */
  dismissed?: boolean;
};

export const SCOPE_SERVICE = 1;
export const SCOPE_CLASH_MODE = 2;
export const SCOPE_RULESET = 3;
export const SCOPE_SYSTEM = 4;
export const SCOPE_CONNECTION_ASK = 5;

export const SCOPE_LABELS: Record<number, string> = {
  [SCOPE_SERVICE]: "Core",
  [SCOPE_CLASH_MODE]: "Mode",
  [SCOPE_RULESET]: "Ruleset",
  [SCOPE_SYSTEM]: "System",
  [SCOPE_CONNECTION_ASK]: "Connection Ask",
};

const MAX = 100;
const TOAST_COOLDOWN_MS = 60_000;
const STORAGE_KEY = "zeytun-notifications";
const MUTE_KEY = "zeytun-notification-mutes";
const RULESET_STATUS_KEY = "zeytun-ruleset-status";

type RulesetLive = {
  status: "ready" | "failed" | "downloading";
  ts_ms: number;
  error?: string | null;
};

type PersistShape = {
  items: CoreNotification[];
};

class NotificationStore {
  items = $state<CoreNotification[]>([]);
  /** Muted scopes: no toast + no unread bump (still stored, filterable in UI). */
  mutedScopes = $state<number[]>([]);
  /**
   * Live ruleset download state — independent of notification history.
   * Clear() must not wipe this or badges flip to Pending.
   */
  rulesetLive = $state<Record<string, RulesetLive>>({});
  #unlisten: UnlistenFn | null = null;
  #toastAt = new Map<string, number>();
  #hydrated = false;

  get unreadCount() {
    return this.items.filter(
      (i) => !i.read && !this.isMuted(i.scope) && i.severity >= 1,
    ).length;
  }

  /** CRITICAL not dismissed — sticky banner. */
  get stickyCritical(): CoreNotification[] {
    return this.items.filter(
      (i) => i.severity >= 3 && !i.dismissed && !this.isMuted(i.scope),
    );
  }

  get visibleItems(): CoreNotification[] {
    // INFO stays in history but center defaults to warn+ (and unmuted).
    // Ruleset rows are aggregated into a single live summary (rulesetSummary)
    // — no per-tag rows with tags in the bell.
    return this.items.filter(
      (i) => !this.isMuted(i.scope),
    );
  }

  /**
   * One aggregated ruleset row for the bell, derived from live download state
   * (never from clearable history). No tags, no per-ruleset noise.
   */
  get rulesetSummary(): { severity: number; title: string; subtitle: string; failedCount: number } | null {
    return null; // Disabled summary array in favor of individual ruleset titles
  }

  isMuted(scope: number): boolean {
    return this.mutedScopes.includes(scope);
  }

  toggleMute(scope: number) {
    if (this.isMuted(scope)) {
      this.mutedScopes = this.mutedScopes.filter((s) => s !== scope);
    } else {
      this.mutedScopes = [...this.mutedScopes, scope];
    }
    this.#saveMutes();
  }

  /** Live map only — never derived from clearable history. */
  rulesetStatus(tag: string): "ready" | "failed" | "downloading" | "unknown" {
    return this.rulesetLive[tag]?.status ?? "unknown";
  }

  rulesetError(tag: string): string | null {
    const live = this.rulesetLive[tag];
    if (!live || live.status !== "failed") return null;
    return live.error ?? null;
  }

  /** Optimistic: Retry / first publish — badge Downloading until core READY/FAIL. */
  markRemoteDownloading(tags: string[]) {
    if (tags.length === 0) return;
    const ts = Date.now();
    const next = { ...this.rulesetLive };
    for (const tag of tags) {
      next[tag] = { status: "downloading", ts_ms: ts, error: null };
    }
    this.rulesetLive = next;
    this.#persistRulesetLive();
  }

  #applyRulesetEvent(p: Omit<CoreNotification, "read" | "dismissed">) {
    if (p.scope !== SCOPE_RULESET) return;
    const tag = p.attrs?.tag;
    if (!tag) return;
    const ts = p.ts_ms || Date.now();
    const prev = this.rulesetLive[tag];
    if (prev && ts < prev.ts_ms) return; // out-of-order

    if (p.code === "RULESET_READY" || p.code === "RULESET_UPDATED") {
      this.rulesetLive = {
        ...this.rulesetLive,
        [tag]: { status: "ready", ts_ms: ts, error: null },
      };
      this.#persistRulesetLive();
      return;
    }
    if (
      p.code === "RULESET_INITIAL_FETCH_FAILED" ||
      p.code === "RULESET_UPDATE_FAILED"
    ) {
      // Explicit download attempt (Downloading badge) → Not downloaded on fail.
      // Quiet periodic UPDATE_FAILED after Ready: keep Ready (rules still loaded).
      if (p.code === "RULESET_UPDATE_FAILED" && prev?.status === "ready") {
        this.rulesetLive = {
          ...this.rulesetLive,
          [tag]: { ...prev, ts_ms: ts },
        };
        this.#persistRulesetLive();
        return;
      }
      this.rulesetLive = {
        ...this.rulesetLive,
        [tag]: {
          status: "failed",
          ts_ms: ts,
          error: p.message || p.title || null,
        },
      };
      this.#persistRulesetLive();
    }
  }

  async init() {
    if (this.#unlisten) return;
    this.#hydrate();
    this.#unlisten = await listen<Omit<CoreNotification, "read" | "dismissed">>(
      "core-notification",
      (event) => {
        this.#ingest(event.payload);
      },
    );
  }

  stopListening() {
    this.#unlisten?.();
    this.#unlisten = null;
  }

  #hydrate() {
    if (this.#hydrated || typeof localStorage === "undefined") return;
    this.#hydrated = true;
    try {
      const raw = localStorage.getItem(STORAGE_KEY);
      if (raw) {
        const parsed = JSON.parse(raw) as PersistShape;
        if (Array.isArray(parsed.items)) {
          this.items = parsed.items.slice(0, MAX);
        }
      }
      const mutes = localStorage.getItem(MUTE_KEY);
      if (mutes) {
        const arr = JSON.parse(mutes) as number[];
        if (Array.isArray(arr)) this.mutedScopes = arr;
      }
      const liveRaw = localStorage.getItem(RULESET_STATUS_KEY);
      if (liveRaw) {
        const live = JSON.parse(liveRaw) as Record<string, RulesetLive>;
        if (live && typeof live === "object") this.rulesetLive = live;
      }
      // Rebuild live map from history if empty (first run after upgrade).
      // Oldest→newest so READY before a later UPDATE_FAILED sticks.
      if (Object.keys(this.rulesetLive).length === 0) {
        const ordered = [...this.items].sort(
          (a, b) => (a.ts_ms ?? 0) - (b.ts_ms ?? 0),
        );
        for (const n of ordered) this.#applyRulesetEvent(n);
      }
    } catch {
      // ignore corrupt storage
    }
  }

  #persist() {
    if (typeof localStorage === "undefined") return;
    try {
      localStorage.setItem(
        STORAGE_KEY,
        JSON.stringify({ items: this.items.slice(0, MAX) } satisfies PersistShape),
      );
    } catch {
      // quota
    }
  }

  #persistRulesetLive() {
    if (typeof localStorage === "undefined") return;
    try {
      // Don't durable-store in-flight Downloading (reload → Pending until event).
      const durable: Record<string, RulesetLive> = {};
      for (const [tag, v] of Object.entries(this.rulesetLive)) {
        if (v.status === "downloading") continue;
        durable[tag] = v;
      }
      localStorage.setItem(RULESET_STATUS_KEY, JSON.stringify(durable));
    } catch {
      // quota
    }
  }

  #saveMutes() {
    if (typeof localStorage === "undefined") return;
    try {
      localStorage.setItem(MUTE_KEY, JSON.stringify(this.mutedScopes));
    } catch {
      // ignore
    }
  }

  #ingest(p: Omit<CoreNotification, "read" | "dismissed">) {
    const item: CoreNotification = {
      id: p.id || crypto.randomUUID(),
      ts_ms: p.ts_ms || Date.now(),
      scope: p.scope ?? 0,
      code: p.code ?? "",
      severity: p.severity ?? 0,
      title: p.title ?? "",
      message: p.message ?? "",
      attrs: p.attrs ?? {},
      read: false,
      dismissed: false,
    };

    this.#applyRulesetEvent(item);

    // Collapse SERVICE / CLASH_MODE to single latest logical row (keep id of new)
    let next = this.items.filter((x) => x.id !== item.id);
    if (item.scope === SCOPE_SERVICE) {
      next = next.filter((x) => x.scope !== SCOPE_SERVICE || x.severity >= 3);
    } else if (item.scope === SCOPE_CLASH_MODE) {
      next = next.filter((x) => x.scope !== SCOPE_CLASH_MODE);
    } else if (item.scope === SCOPE_RULESET && item.attrs?.tag) {
      // Collapse previous events for the same ruleset tag so history doesn't explode
      next = next.filter((x) => x.scope !== SCOPE_RULESET || x.attrs?.tag !== item.attrs.tag);
    }

    this.items = [item, ...next].slice(0, MAX);
    this.#persist();
    this.#maybeToast(item);
  }

  #maybeToast(item: CoreNotification) {
    // Severity policy: INFO silent; WARN no toast; ERROR toast; CRITICAL sticky+toast
    if (this.isMuted(item.scope)) return;
    if (item.severity < 2) return;
    if (item.code === "SERVICE_STARTED" || item.code === "SERVICE_IDLE") return;

    const key = `${item.code}|${item.attrs?.tag ?? item.attrs?.status ?? ""}`;
    const now = Date.now();
    const last = this.#toastAt.get(key) ?? 0;
    if (now - last < TOAST_COOLDOWN_MS) return;
    this.#toastAt.set(key, now);

    if (item.severity >= 3) {
      toast.error(item.title || item.code, {
        description: item.message || "Requires attention",
        duration: Infinity,
      });
    } else {
      toast.error(item.title || item.code, {
        description: item.message || undefined,
      });
    }
  }

  latestServiceStatus(): string | null {
    for (const n of this.items) {
      if (n.scope === SCOPE_SERVICE && n.attrs?.status) return n.attrs.status;
    }
    return null;
  }

  latestClashMode(): string | null {
    for (const n of this.items) {
      if (n.scope === SCOPE_CLASH_MODE && (n.attrs?.mode || n.message))
        return n.attrs?.mode || n.message;
    }
    return null;
  }

  markAllRead() {
    this.items = this.items.map((i) => ({ ...i, read: true }));
    this.#persist();
  }

  markRead(id: string) {
    this.items = this.items.map((i) =>
      i.id === id ? { ...i, read: true } : i,
    );
    this.#persist();
  }

  dismissCritical(id: string) {
    this.items = this.items.map((i) =>
      i.id === id ? { ...i, dismissed: true, read: true } : i,
    );
    this.#persist();
  }

  dismissAllCritical() {
    this.items = this.items.map((i) =>
      i.severity >= 3 ? { ...i, dismissed: true, read: true } : i,
    );
    this.#persist();
  }

  clear() {
    // Keep undismissed critical until user dismisses.
    // rulesetLive untouched — badge state is not notification history.
    this.items = this.items.filter((i) => i.severity >= 3 && !i.dismissed);
    this.#persist();
  }

  clearAll() {
    this.items = [];
    this.#persist();
  }
}

export const notificationStore = new NotificationStore();
