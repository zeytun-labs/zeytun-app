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
  pendingRestart = $state(false);
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

  #unlistenProfile: UnlistenFn | null = null;
  #unlistenStatus: UnlistenFn | null = null;

  outboundMode = $derived<OutboundMode>(
    this.profile?.outbound_mode ?? "direct",
  );

  /// The currently-active profile's registry entry.
  activeProfile = $derived<ProfileMeta | null>(
    this.profiles.find((p) => p.is_active) ?? null,
  );

  /// Does the active profile carry a subscription URL?
  hasSubscription = $derived(
    Boolean(this.activeProfile?.subscription_url?.trim()),
  );

  visiblePolicies = $derived.by(() => {
    if (!this.profile) return [];
    return this.profile.policies.filter((p) => p.tag !== DEFAULT_POLICY_TAG);
  });


  async load() {
    this.loading = true;
    try {
      const [nextProfile, nextRuntime, profiles] = await Promise.all([
        coreProfileGet(),
        coreStatus(),
        coreProfileList(),
      ]);
      this.profile = nextProfile;
      this.runtime = nextRuntime;
      this.profiles = profiles;
      // Bootstrap the indicator in case the daemon's status emit fired before
      // the listener was registered; daemon events are authoritative afterwards.
      if (nextRuntime.phase === "running") this.coreStatus = "running";
    } finally {
      this.loading = false;
    }
  }

  async refresh() {
    const [nextProfile, nextRuntime, profiles] = await Promise.all([
      coreProfileGet(),
      coreStatus(),
      coreProfileList(),
    ]);
    this.profile = nextProfile;
    this.runtime = nextRuntime;
    this.profiles = profiles;
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

  /// Re-fetch the profile's subscription. Foreground (background=false) returns
  /// the summary for immediate display.
  async refreshSubscription(id: string): Promise<SyncSummary> {
    const summary = await coreProfileRefresh(id, false);
    await this.refresh();
    return summary;
  }

  async markSummaryRead(id: string) {
    await coreProfileMarkSummaryRead(id);
    await this.refresh();
  }

  /// Clear the pending restart flag (called after user clicks Restart Core).
  async clearPendingRestart() {
    this.pendingRestart = false;
    // Also refresh to get the latest core status
    await this.refresh();
  }

  /// Subscribe to backend sync events. Idempotent. Returns nothing — pair with
  /// stopListening() on teardown. `profile-updated` (tray/command mutations)
  /// re-syncs the profile; `core-status-changed` (daemon link) drives the
  /// status indicator.
  async startListening() {
    if (!this.#unlistenProfile) {
      this.#unlistenProfile = await listen("profile-updated", () => {
        void this.refresh();
      });
    }
    if (!this.#unlistenStatus) {
      this.#unlistenStatus = await listen<CoreStatus>(
        "core-status-changed",
        (event) => {
          this.coreStatus = event.payload;
        },
      );
    }
  }

  /// Synchronous teardown of the event listeners.
  stopListening() {
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
