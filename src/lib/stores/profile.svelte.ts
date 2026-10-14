import {
  coreProfileGet,
  coreProfileList,
  coreProfileCreate,
  coreProfileUpdate,
  coreProfileDelete,
  coreProfileSwitch,
  coreProfileRefresh,
  coreProfileMarkSummaryRead,
  coreStatus,
} from "$lib/core/api";
import type {
  CoreProfile,
  CoreRuntimeStatus,
  CoreStatus,
  CreateProfileInput,
  OutboundMode,
  ProfileMeta,
  Proxy,
  ProxyPolicy,
  SyncSummary,
  UpdateProfileInput,
} from "$lib/core/types";
import { DEFAULT_POLICY_TAG } from "$lib/core/constants";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { runNetworkToggle } from "./network-toggles";

class ProfileStore {
  profile = $state<CoreProfile | null>(null);
  runtime = $state<CoreRuntimeStatus | null>(null);
  loading = $state(true);

  /// The profile registry (all profiles + active flag + unread summary).
  profiles = $state<ProfileMeta[]>([]);

  /// True while a profile switch is in flight (backend regen + core restart).
  /// The layout uses this to show the full-page loading overlay.
  switching = $state(false);

  /// Live core status from the daemon gRPC link — the source of truth for the
  /// status indicator. Seeded "connecting" until the first event arrives.
  coreStatus = $state<CoreStatus>("connecting");

  /// True when a core restart is required for changes to take effect.
  /// Set by control-center/settings when user toggles something needing restart.
  /// Cleared when user clicks "Restart Core" in topbar.
  get pendingRestart() { return this.runtime?.pending_apply ?? false; }
  set pendingRestart(_requested: boolean) {
    // Existing setters request a read-back, never invent applied/saved state.
    void this.refresh().catch((error) => console.error("Profile refresh failed", error));
  }
  networkToggle = $state<"systemProxy" | "tun" | null>(null);

  async toggleNetwork(
    kind: "systemProxy" | "tun",
    action: () => Promise<void>,
  ) {
    await runNetworkToggle(
      () => this.networkToggle,
      (value) => {
        this.networkToggle = value;
      },
      action,
      () => this.refresh(),
    )(kind);
  }

  isRunning = $derived(this.coreStatus === "running");

  #requestGeneration = 0;
  #statusGeneration = 0;
  #listenerGeneration = 0;
  #registration: Promise<void> | null = null;
  #pendingCleanup: (() => void) | null = null;

  #unlistenProfile: UnlistenFn | null = null;
  #unlistenStatus: UnlistenFn | null = null;

  outboundMode = $derived<OutboundMode>(
    this.profile?.outbound_mode ?? "direct",
  );

  /// The currently-active profile's registry entry.
  activeProfile = $derived<ProfileMeta | null>(
    this.profiles.find((p) => p.is_active) ?? null,
  );

  get activeProfileId(): string | null {
    return this.activeProfile?.id ?? null;
  }

  visiblePolicies = $derived.by(() => {
    if (!this.profile) return [];
    return this.profile.policies.filter((p) => p.tag !== DEFAULT_POLICY_TAG);
  });


  async load() {
    this.loading = true;
    await this.#readSnapshot(true);
  }

  async refresh() {
    await this.#readSnapshot(false);
  }

  async #readSnapshot(bootstrap: boolean) {
    const generation = ++this.#requestGeneration;
    try {
      const [nextProfile, nextRuntime, profiles] = await Promise.all([
        coreProfileGet(),
        coreStatus(),
        coreProfileList(),
      ]);
      if (generation !== this.#requestGeneration) return;
      this.profile = nextProfile;
      this.runtime = nextRuntime;
      this.profiles = profiles;
      // A live daemon event received during this read takes precedence.
      if (bootstrap && this.#statusGeneration === 0 && nextRuntime.phase === "running") {
        this.coreStatus = "running";
      }
    } finally {
      if (generation === this.#requestGeneration) this.loading = false;
    }
  }

  // --- Profile registry actions ---

  async switchProfile(id: string) {
    // Drives the layout's full-page loading overlay: backend regenerates the
    // config and restarts the core, which is the source of the perceived freeze.
    this.switching = true;
    try {
      await coreProfileSwitch(id);
      await this.refresh();
    } finally {
      this.switching = false;
    }
  }

  async createProfile(input: CreateProfileInput): Promise<ProfileMeta> {
    const meta = await coreProfileCreate(input);
    await this.refresh();
    return meta;
  }

  async updateProfile(input: UpdateProfileInput): Promise<ProfileMeta> {
    const meta = await coreProfileUpdate(input);
    await this.refresh();
    return meta;
  }

  async deleteProfile(id: string) {
    await coreProfileDelete(id);
    await this.refresh();
  }

  async markSummaryRead(id: string) {
    await coreProfileMarkSummaryRead(id);
    await this.refresh();
  }

  /// Clear the pending restart flag (called after user clicks Restart Core).
  async clearPendingRestart() {
    // Also refresh to get the latest core status
    await this.refresh();
  }

  /// Subscribe to backend sync events. Idempotent. Returns nothing — pair with
  /// stopListening() on teardown. `profile-updated` (tray/command mutations)
  /// re-syncs the profile; `core-status-changed` (daemon link) drives the
  /// status indicator.
  startListening(): Promise<void> {
    if (this.#registration) return this.#registration;
    if (this.#unlistenProfile && this.#unlistenStatus) return Promise.resolve();
    const generation = this.#listenerGeneration;
    this.#registration = this.#registerListeners(generation)
      .catch((error) => console.error("Profile listener setup failed", error))
      .finally(() => {
        if (generation === this.#listenerGeneration) this.#registration = null;
      });
    return this.#registration;
  }

  async #registerListeners(generation: number) {
    const handles: UnlistenFn[] = [];
    const cleanup = () => handles.splice(0).forEach((unlisten) => unlisten());
    this.#pendingCleanup = cleanup;
    let committed = false;
    try {
      handles.push(await listen("profile-updated", () => {
        if (generation !== this.#listenerGeneration) return;
        void this.refresh().catch((error) => console.error("Profile refresh failed", error));
      }));
      if (generation !== this.#listenerGeneration) return;
      handles.push(await listen<CoreStatus>("core-status-changed", (event) => {
        if (generation !== this.#listenerGeneration) return;
        ++this.#statusGeneration;
        this.coreStatus = event.payload;
      }));
      if (generation !== this.#listenerGeneration) return;
      [this.#unlistenProfile, this.#unlistenStatus] = handles;
      committed = true;
    } finally {
      if (!committed) cleanup();
      if (this.#pendingCleanup === cleanup) this.#pendingCleanup = null;
    }
  }

  /// Invalidate pending registrations; their late handles are released on arrival.
  stopListening() {
    ++this.#listenerGeneration;
    this.#pendingCleanup?.();
    this.#pendingCleanup = null;
    this.#registration = null;
    this.#unlistenProfile?.();
    this.#unlistenProfile = null;
    this.#unlistenStatus?.();
    this.#unlistenStatus = null;
  }

  /// All proxies in the active profile.
  get proxies(): Proxy[] {
    return this.profile?.proxies ?? [];
  }

  getMemberName(memberTag: string): string {
    if (!this.profile) return memberTag;
    const proxy = this.profile.proxies.find((item) => item.tag === memberTag);
    if (proxy) return proxy.title;
    const policy = this.profile.policies.find(
      (item) => item.tag === memberTag,
    );
    return policy?.name ?? memberTag;
  }

  getPolicyMembers(policy: ProxyPolicy): { tag: string; name: string }[] {
    return (policy.members ?? []).map((tag) => ({
      tag,
      name: this.getMemberName(tag),
    }));
  }

  getDefaultPolicy(): ProxyPolicy | null {
    if (!this.profile) return null;
    return (
      this.profile.policies.find(
        (policy) => policy.tag === DEFAULT_POLICY_TAG,
      ) ?? null
    );
  }
}

export const profileStore = new ProfileStore();
