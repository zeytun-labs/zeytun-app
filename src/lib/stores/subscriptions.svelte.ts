import {
  coreSubscriptionList,
  coreSubscriptionCreate,
  coreSubscriptionUpdate,
  coreSubscriptionRefresh,
  coreSubscriptionRefreshAll,
  coreSubscriptionDeletePreview,
  coreSubscriptionDelete,
  coreSubscriptionMarkSummaryRead,
} from "$lib/core/api";
import type {
  SubscriptionSource,
  SubscriptionSettings,
  DeleteSubscriptionInput,
  SubscriptionDeletePreview,
  RefreshResult,
} from "$lib/core/types";
import { profileStore } from "./profile.svelte";

class SubscriptionsStore {
  sources = $state<SubscriptionSource[]>([]);
  loading = $state(false);
  refreshingSourceIds = $state<Record<string, boolean>>({});
  refreshingAll = $state(false);
  error = $state<string | null>(null);
  activeProfileId = $state<string | null>(null);

  isRefreshing(sourceId: string): boolean {
    return Boolean(this.refreshingSourceIds[sourceId]) || this.refreshingAll;
  }

  async load(profileId?: string): Promise<void> {
    const targetId = profileId ?? profileStore.activeProfileId;
    if (!targetId) {
      this.sources = [];
      this.activeProfileId = null;
      return;
    }

    this.activeProfileId = targetId;
    this.loading = true;
    this.error = null;
    try {
      const list = await coreSubscriptionList(targetId);
      // Defensive check against stale profile response
      if (this.activeProfileId === targetId) {
        this.sources = list;
      }
    } catch (e: any) {
      if (this.activeProfileId === targetId) {
        this.error = e?.message ?? String(e);
      }
    } finally {
      if (this.activeProfileId === targetId) {
        this.loading = false;
      }
    }
  }

  async create(settings: SubscriptionSettings, profileId?: string): Promise<SubscriptionSource> {
    const targetId = profileId ?? profileStore.activeProfileId;
    if (!targetId) throw new Error("No active profile");

    const created = await coreSubscriptionCreate(targetId, settings);
    await this.load(targetId);
    return created;
  }

  async update(sourceId: string, settings: SubscriptionSettings, profileId?: string): Promise<SubscriptionSource> {
    const targetId = profileId ?? profileStore.activeProfileId;
    if (!targetId) throw new Error("No active profile");

    const updated = await coreSubscriptionUpdate(targetId, sourceId, settings);
    await this.load(targetId);
    return updated;
  }

  async toggleEnabled(source: SubscriptionSource, profileId?: string): Promise<void> {
    const targetId = profileId ?? profileStore.activeProfileId;
    if (!targetId) throw new Error("No active profile");

    const newSettings: SubscriptionSettings = {
      ...source.settings,
      enabled: !source.settings.enabled,
    };
    await this.update(source.id, newSettings, targetId);
  }

  async refresh(sourceId: string, profileId?: string): Promise<RefreshResult> {
    const targetId = profileId ?? profileStore.activeProfileId;
    if (!targetId) throw new Error("No active profile");

    this.refreshingSourceIds = { ...this.refreshingSourceIds, [sourceId]: true };
    try {
      const result = await coreSubscriptionRefresh(targetId, sourceId);
      return result;
    } finally {
      const next = { ...this.refreshingSourceIds };
      delete next[sourceId];
      this.refreshingSourceIds = next;
      await this.load(targetId);
      await profileStore.refresh();
    }
  }

  async refreshAll(profileId?: string): Promise<RefreshResult[]> {
    const targetId = profileId ?? profileStore.activeProfileId;
    if (!targetId) throw new Error("No active profile");

    this.refreshingAll = true;
    try {
      const results = await coreSubscriptionRefreshAll(targetId);
      return results;
    } finally {
      this.refreshingAll = false;
      await this.load(targetId);
      await profileStore.refresh();
    }
  }

  async deletePreview(sourceId: string, profileId?: string): Promise<SubscriptionDeletePreview> {
    const targetId = profileId ?? profileStore.activeProfileId;
    if (!targetId) throw new Error("No active profile");

    return await coreSubscriptionDeletePreview(targetId, sourceId);
  }

  async delete(input: DeleteSubscriptionInput): Promise<void> {
    await coreSubscriptionDelete(input);
    await this.load(input.profile_id);
    await profileStore.refresh();
  }

  async markSummaryRead(sourceId: string, profileId?: string): Promise<void> {
    const targetId = profileId ?? profileStore.activeProfileId;
    if (!targetId) return;

    await coreSubscriptionMarkSummaryRead(targetId, sourceId);
    const source = this.sources.find((s) => s.id === sourceId);
    if (source) {
      source.unread_summary = false;
    }
  }
}

export const subscriptionsStore = new SubscriptionsStore();
