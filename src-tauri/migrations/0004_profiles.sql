-- Move from a single flat config to multiple Profiles. Each profile owns its
-- own routing bundle (proxies/policies/rules/dns/rule_sets + outbound_mode);
-- local_proxy_config and traffic_analytics stay global. The old subscription
-- "group" concept is removed — a profile carries a single optional
-- subscription_url and proxies are tagged manual vs subscription via `origin`.
--
-- Columns are ADDED (never dropped) so this is safe on older SQLite and
-- preserves all existing rows. Legacy proxy.group_id / proxy_group / group_item
-- are left in place but unused.

CREATE TABLE IF NOT EXISTS profile (
  id                   TEXT    PRIMARY KEY NOT NULL,
  name                 TEXT    NOT NULL,
  subscription_url     TEXT,
  last_updated_unix_ms INTEGER,
  is_active            INTEGER NOT NULL DEFAULT 0,
  outbound_mode        TEXT    NOT NULL DEFAULT 'rule',
  version              TEXT    NOT NULL DEFAULT '0.0.1',
  last_sync_summary    TEXT,               -- JSON-encoded SyncSummary
  unread_summary       INTEGER NOT NULL DEFAULT 0,
  position             INTEGER NOT NULL DEFAULT 0
);

-- Scope the per-profile config tables. Default 'default' back-fills existing
-- single-profile rows onto the migrated Default profile.
ALTER TABLE proxy         ADD COLUMN profile_id TEXT NOT NULL DEFAULT 'default';
ALTER TABLE proxy         ADD COLUMN origin     TEXT NOT NULL DEFAULT 'manual';
ALTER TABLE policy        ADD COLUMN profile_id TEXT NOT NULL DEFAULT 'default';
ALTER TABLE policy_member ADD COLUMN profile_id TEXT NOT NULL DEFAULT 'default';
ALTER TABLE route_rule    ADD COLUMN profile_id TEXT NOT NULL DEFAULT 'default';
ALTER TABLE route_rule    ADD COLUMN orphaned   INTEGER NOT NULL DEFAULT 0;
ALTER TABLE rule_set      ADD COLUMN profile_id TEXT NOT NULL DEFAULT 'default';
ALTER TABLE dns_config    ADD COLUMN profile_id TEXT NOT NULL DEFAULT 'default';
ALTER TABLE dns_server    ADD COLUMN profile_id TEXT NOT NULL DEFAULT 'default';

-- Seed / migrate the Default profile. Idempotent: INSERT OR IGNORE keyed on id.
-- COALESCE pulls existing settings for upgrading users; falls back to defaults
-- on a fresh install (empty setting table).
INSERT OR IGNORE INTO profile (id, name, subscription_url, is_active, outbound_mode, version, position)
SELECT 'default',
       'Default',
       NULL,
       1,
       COALESCE((SELECT outbound_mode FROM setting LIMIT 1), 'rule'),
       COALESCE((SELECT version FROM setting LIMIT 1), '0.0.1'),
       0;

-- Classify existing proxies: those that came from a subscription group (group
-- had a non-empty url) become 'subscription'; everything else stays 'manual'.
UPDATE proxy
   SET origin = 'subscription'
 WHERE group_id IN (
   SELECT id FROM proxy_group WHERE url IS NOT NULL AND trim(url) <> ''
 );

CREATE INDEX IF NOT EXISTS idx_proxy_profile      ON proxy(profile_id);
CREATE INDEX IF NOT EXISTS idx_policy_profile     ON policy(profile_id);
CREATE INDEX IF NOT EXISTS idx_policy_member_prof ON policy_member(profile_id);
CREATE INDEX IF NOT EXISTS idx_route_rule_profile ON route_rule(profile_id);
CREATE INDEX IF NOT EXISTS idx_rule_set_profile   ON rule_set(profile_id);
