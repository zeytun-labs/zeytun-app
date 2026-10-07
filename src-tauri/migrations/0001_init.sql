-- Zeytun SQLite Database Schema v1 (Clean baseline)

CREATE TABLE IF NOT EXISTS setting (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  version INTEGER NOT NULL,
  outbound_mode TEXT NOT NULL,
  selected_policy_tag TEXT,
  selected_proxy_tag TEXT,
  updated_unix_ms INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS local_proxy_config (
  mode TEXT,
  listen TEXT NOT NULL,
  socks_port INTEGER NOT NULL,
  http_port INTEGER NOT NULL,
  mixed_port INTEGER,
  tun_name TEXT,
  tun_mtu INTEGER,
  tun_auto_route INTEGER,
  system_proxy INTEGER,
  log_level TEXT,
  connection_ask_enabled INTEGER NOT NULL DEFAULT 0,
  connection_ask_timeout_ms INTEGER NOT NULL DEFAULT 60000,
  connection_ask_group_by TEXT NOT NULL DEFAULT 'process',
  connection_ask_remember_default INTEGER NOT NULL DEFAULT 1
);

CREATE TABLE IF NOT EXISTS traffic_analytics (
  ts_minute  INTEGER NOT NULL,
  domain     TEXT    NOT NULL,
  policy     TEXT    NOT NULL,
  process    TEXT    NOT NULL,
  is_proxy   INTEGER NOT NULL,
  up_bytes   INTEGER NOT NULL DEFAULT 0,
  down_bytes INTEGER NOT NULL DEFAULT 0,
  PRIMARY KEY (ts_minute, domain, policy, process, is_proxy)
) WITHOUT ROWID;

CREATE INDEX IF NOT EXISTS idx_traffic_ts ON traffic_analytics (ts_minute);

CREATE TABLE IF NOT EXISTS network_policy (
  traffic TEXT PRIMARY KEY,
  policy  TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS profile (
  id                   TEXT    PRIMARY KEY NOT NULL,
  name                 TEXT    NOT NULL,
  icon                 TEXT,
  is_active            INTEGER NOT NULL DEFAULT 0,
  outbound_mode        TEXT    NOT NULL DEFAULT 'rule',
  version              TEXT    NOT NULL DEFAULT '0.0.1',
  position             INTEGER NOT NULL DEFAULT 0,
  config_revision      INTEGER NOT NULL DEFAULT 0,
  graph_revision       INTEGER NOT NULL DEFAULT 0,
  subscription_url     TEXT,
  last_updated_unix_ms INTEGER,
  last_sync_summary    TEXT,
  unread_summary       INTEGER NOT NULL DEFAULT 0,
  skip_auto_update     INTEGER NOT NULL DEFAULT 0,
  update_interval_hours INTEGER NOT NULL DEFAULT 12,
  sub_upload           INTEGER,
  sub_download         INTEGER,
  sub_total            INTEGER,
  sub_expire           INTEGER
);

CREATE TABLE IF NOT EXISTS subscription_source (
  profile_id TEXT NOT NULL,
  id TEXT NOT NULL,
  name TEXT NOT NULL,
  url TEXT NOT NULL,
  enabled INTEGER NOT NULL CHECK(enabled IN (0,1)),
  auto_update INTEGER NOT NULL CHECK(auto_update IN (0,1)),
  update_interval_hours INTEGER NOT NULL CHECK(update_interval_hours > 0),
  generation INTEGER NOT NULL CHECK(generation >= 0),
  position INTEGER NOT NULL,
  provider_title TEXT,
  suggested_interval_hours INTEGER,
  userinfo_json TEXT,
  last_attempt_at INTEGER,
  last_success_at INTEGER,
  last_error TEXT,
  last_sync_summary_json TEXT,
  unread_summary INTEGER NOT NULL DEFAULT 0 CHECK(unread_summary IN (0,1)),
  PRIMARY KEY(profile_id, id),
  FOREIGN KEY(profile_id) REFERENCES profile(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS proxy (
  profile_id TEXT NOT NULL,
  tag TEXT NOT NULL,
  origin TEXT NOT NULL,
  source_id TEXT,
  available INTEGER NOT NULL DEFAULT 1 CHECK(available IN (0,1)),
  title TEXT NOT NULL,
  protocol TEXT NOT NULL,
  link TEXT NOT NULL,
  enabled INTEGER NOT NULL,
  transport TEXT,
  config_json TEXT,
  position INTEGER NOT NULL,
  PRIMARY KEY(profile_id, tag),
  CHECK((origin = 'manual' AND source_id IS NULL) OR (origin = 'subscription' AND source_id IS NOT NULL)),
  FOREIGN KEY(profile_id) REFERENCES profile(id) ON DELETE CASCADE,
  FOREIGN KEY(profile_id, source_id) REFERENCES subscription_source(profile_id, id)
);

CREATE INDEX IF NOT EXISTS idx_proxy_profile ON proxy(profile_id, position);
CREATE INDEX IF NOT EXISTS idx_proxy_source ON proxy(profile_id, source_id);

CREATE TABLE IF NOT EXISTS policy (
  profile_id          TEXT    NOT NULL,
  tag                 TEXT    NOT NULL,
  name                TEXT    NOT NULL,
  kind                TEXT,
  selected_member_tag TEXT,
  test_url            TEXT,
  interval_seconds    INTEGER,
  tolerance_ms        INTEGER,
  position            INTEGER NOT NULL,
  strategy            TEXT,
  weights_json        TEXT,
  PRIMARY KEY(profile_id, tag),
  FOREIGN KEY(profile_id) REFERENCES profile(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_policy_profile ON policy(profile_id, position);

CREATE TABLE IF NOT EXISTS policy_member (
  profile_id  TEXT    NOT NULL,
  policy_tag  TEXT    NOT NULL,
  member_tag  TEXT    NOT NULL,
  position    INTEGER NOT NULL,
  PRIMARY KEY(profile_id, policy_tag, member_tag),
  FOREIGN KEY(profile_id) REFERENCES profile(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_policy_member_prof ON policy_member(profile_id, policy_tag, position);

CREATE TABLE IF NOT EXISTS route_rule (
  profile_id TEXT    NOT NULL,
  id         INTEGER NOT NULL,
  kind       TEXT    NOT NULL,
  value      TEXT    NOT NULL,
  outbound   TEXT    NOT NULL,
  enabled    INTEGER NOT NULL,
  comment    TEXT    NOT NULL,
  rule_set   TEXT,
  orphaned   INTEGER NOT NULL DEFAULT 0,
  position   INTEGER NOT NULL,
  PRIMARY KEY(profile_id, id),
  FOREIGN KEY(profile_id) REFERENCES profile(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_route_rule_profile ON route_rule(profile_id, position);

CREATE TABLE IF NOT EXISTS temp_route_rule (
  profile_id TEXT NOT NULL REFERENCES profile(id) ON DELETE CASCADE,
  id INTEGER NOT NULL,
  kind TEXT NOT NULL,
  value TEXT NOT NULL,
  outbound TEXT NOT NULL,
  comment TEXT NOT NULL DEFAULT '',
  rule_set TEXT,
  orphaned INTEGER NOT NULL DEFAULT 0,
  expires_at INTEGER NOT NULL,
  position INTEGER NOT NULL,
  enabled INTEGER NOT NULL DEFAULT 1,
  PRIMARY KEY(profile_id, id)
);

CREATE TABLE IF NOT EXISTS rule_set (
  profile_id TEXT NOT NULL REFERENCES profile(id) ON DELETE CASCADE,
  id TEXT NOT NULL,
  tag TEXT NOT NULL,
  type TEXT NOT NULL,
  source TEXT NOT NULL,
  action TEXT NOT NULL,
  comment TEXT,
  position INTEGER NOT NULL,
  download_policy TEXT,
  enabled INTEGER NOT NULL DEFAULT 1,
  name TEXT,
  PRIMARY KEY(profile_id, id)
);

CREATE TABLE IF NOT EXISTS dns_config (
  profile_id   TEXT NOT NULL,
  final_server TEXT,
  fake_ip      INTEGER,
  PRIMARY KEY(profile_id),
  FOREIGN KEY(profile_id) REFERENCES profile(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS dns_server (
  profile_id TEXT    NOT NULL,
  tag        TEXT    NOT NULL,
  address    TEXT    NOT NULL,
  detour     TEXT,
  position   INTEGER NOT NULL,
  name       TEXT,
  PRIMARY KEY(profile_id, tag),
  FOREIGN KEY(profile_id) REFERENCES profile(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_dns_server_profile ON dns_server(profile_id, position);

CREATE TABLE IF NOT EXISTS dns_rule (
  profile_id TEXT NOT NULL REFERENCES profile(id) ON DELETE CASCADE,
  id TEXT NOT NULL,
  kind TEXT NOT NULL,
  value TEXT NOT NULL,
  target TEXT NOT NULL,
  comment TEXT,
  enabled INTEGER NOT NULL DEFAULT 1,
  position INTEGER NOT NULL,
  PRIMARY KEY(profile_id, id)
);

CREATE TABLE IF NOT EXISTS dns_host (
  profile_id TEXT NOT NULL REFERENCES profile(id) ON DELETE CASCADE,
  id TEXT NOT NULL,
  domain TEXT NOT NULL,
  address TEXT NOT NULL,
  enabled INTEGER NOT NULL DEFAULT 1,
  position INTEGER NOT NULL,
  PRIMARY KEY(profile_id, id)
);

CREATE INDEX IF NOT EXISTS idx_dns_host_profile ON dns_host(profile_id, position);

-- Initial default profile seed
INSERT OR IGNORE INTO profile (id, name, is_active, outbound_mode, version, position)
VALUES ('default', 'Default', 1, 'rule', '0.0.1', 0);

-- Graph revision triggers (increment profile.graph_revision on authored changes)
CREATE TRIGGER IF NOT EXISTS graph_revision_proxy_insert AFTER INSERT ON proxy
BEGIN UPDATE profile SET graph_revision = graph_revision + 1 WHERE id IN (NEW.profile_id); END;

CREATE TRIGGER IF NOT EXISTS graph_revision_proxy_update AFTER UPDATE ON proxy
BEGIN UPDATE profile SET graph_revision = graph_revision + 1 WHERE id IN (OLD.profile_id, NEW.profile_id); END;

CREATE TRIGGER IF NOT EXISTS graph_revision_proxy_delete AFTER DELETE ON proxy
BEGIN UPDATE profile SET graph_revision = graph_revision + 1 WHERE id IN (OLD.profile_id); END;

CREATE TRIGGER IF NOT EXISTS graph_revision_policy_insert AFTER INSERT ON policy
BEGIN UPDATE profile SET graph_revision = graph_revision + 1 WHERE id IN (NEW.profile_id); END;

CREATE TRIGGER IF NOT EXISTS graph_revision_policy_update AFTER UPDATE ON policy
BEGIN UPDATE profile SET graph_revision = graph_revision + 1 WHERE id IN (OLD.profile_id, NEW.profile_id); END;

CREATE TRIGGER IF NOT EXISTS graph_revision_policy_delete AFTER DELETE ON policy
BEGIN UPDATE profile SET graph_revision = graph_revision + 1 WHERE id IN (OLD.profile_id); END;

CREATE TRIGGER IF NOT EXISTS graph_revision_policy_member_insert AFTER INSERT ON policy_member
BEGIN UPDATE profile SET graph_revision = graph_revision + 1 WHERE id IN (NEW.profile_id); END;

CREATE TRIGGER IF NOT EXISTS graph_revision_policy_member_update AFTER UPDATE ON policy_member
BEGIN UPDATE profile SET graph_revision = graph_revision + 1 WHERE id IN (OLD.profile_id, NEW.profile_id); END;

CREATE TRIGGER IF NOT EXISTS graph_revision_policy_member_delete AFTER DELETE ON policy_member
BEGIN UPDATE profile SET graph_revision = graph_revision + 1 WHERE id IN (OLD.profile_id); END;

CREATE TRIGGER IF NOT EXISTS graph_revision_route_rule_insert AFTER INSERT ON route_rule
BEGIN UPDATE profile SET graph_revision = graph_revision + 1 WHERE id IN (NEW.profile_id); END;

CREATE TRIGGER IF NOT EXISTS graph_revision_route_rule_update AFTER UPDATE ON route_rule
BEGIN UPDATE profile SET graph_revision = graph_revision + 1 WHERE id IN (OLD.profile_id, NEW.profile_id); END;

CREATE TRIGGER IF NOT EXISTS graph_revision_route_rule_delete AFTER DELETE ON route_rule
BEGIN UPDATE profile SET graph_revision = graph_revision + 1 WHERE id IN (OLD.profile_id); END;

CREATE TRIGGER IF NOT EXISTS graph_revision_temp_route_rule_insert AFTER INSERT ON temp_route_rule
BEGIN UPDATE profile SET graph_revision = graph_revision + 1 WHERE id IN (NEW.profile_id); END;

CREATE TRIGGER IF NOT EXISTS graph_revision_temp_route_rule_update AFTER UPDATE ON temp_route_rule
BEGIN UPDATE profile SET graph_revision = graph_revision + 1 WHERE id IN (OLD.profile_id, NEW.profile_id); END;

CREATE TRIGGER IF NOT EXISTS graph_revision_temp_route_rule_delete AFTER DELETE ON temp_route_rule
BEGIN UPDATE profile SET graph_revision = graph_revision + 1 WHERE id IN (OLD.profile_id); END;

CREATE TRIGGER IF NOT EXISTS graph_revision_rule_set_insert AFTER INSERT ON rule_set
BEGIN UPDATE profile SET graph_revision = graph_revision + 1 WHERE id IN (NEW.profile_id); END;

CREATE TRIGGER IF NOT EXISTS graph_revision_rule_set_update AFTER UPDATE ON rule_set
BEGIN UPDATE profile SET graph_revision = graph_revision + 1 WHERE id IN (OLD.profile_id, NEW.profile_id); END;

CREATE TRIGGER IF NOT EXISTS graph_revision_rule_set_delete AFTER DELETE ON rule_set
BEGIN UPDATE profile SET graph_revision = graph_revision + 1 WHERE id IN (OLD.profile_id); END;

CREATE TRIGGER IF NOT EXISTS graph_revision_dns_config_insert AFTER INSERT ON dns_config
BEGIN UPDATE profile SET graph_revision = graph_revision + 1 WHERE id IN (NEW.profile_id); END;

CREATE TRIGGER IF NOT EXISTS graph_revision_dns_config_update AFTER UPDATE ON dns_config
BEGIN UPDATE profile SET graph_revision = graph_revision + 1 WHERE id IN (OLD.profile_id, NEW.profile_id); END;

CREATE TRIGGER IF NOT EXISTS graph_revision_dns_config_delete AFTER DELETE ON dns_config
BEGIN UPDATE profile SET graph_revision = graph_revision + 1 WHERE id IN (OLD.profile_id); END;

CREATE TRIGGER IF NOT EXISTS graph_revision_dns_server_insert AFTER INSERT ON dns_server
BEGIN UPDATE profile SET graph_revision = graph_revision + 1 WHERE id IN (NEW.profile_id); END;

CREATE TRIGGER IF NOT EXISTS graph_revision_dns_server_update AFTER UPDATE ON dns_server
BEGIN UPDATE profile SET graph_revision = graph_revision + 1 WHERE id IN (OLD.profile_id, NEW.profile_id); END;

CREATE TRIGGER IF NOT EXISTS graph_revision_dns_server_delete AFTER DELETE ON dns_server
BEGIN UPDATE profile SET graph_revision = graph_revision + 1 WHERE id IN (OLD.profile_id); END;

CREATE TRIGGER IF NOT EXISTS graph_revision_dns_rule_insert AFTER INSERT ON dns_rule
BEGIN UPDATE profile SET graph_revision = graph_revision + 1 WHERE id IN (NEW.profile_id); END;

CREATE TRIGGER IF NOT EXISTS graph_revision_dns_rule_update AFTER UPDATE ON dns_rule
BEGIN UPDATE profile SET graph_revision = graph_revision + 1 WHERE id IN (OLD.profile_id, NEW.profile_id); END;

CREATE TRIGGER IF NOT EXISTS graph_revision_dns_rule_delete AFTER DELETE ON dns_rule
BEGIN UPDATE profile SET graph_revision = graph_revision + 1 WHERE id IN (OLD.profile_id); END;

CREATE TRIGGER IF NOT EXISTS graph_revision_dns_host_insert AFTER INSERT ON dns_host
BEGIN UPDATE profile SET graph_revision = graph_revision + 1 WHERE id IN (NEW.profile_id); END;

CREATE TRIGGER IF NOT EXISTS graph_revision_dns_host_update AFTER UPDATE ON dns_host
BEGIN UPDATE profile SET graph_revision = graph_revision + 1 WHERE id IN (OLD.profile_id, NEW.profile_id); END;

CREATE TRIGGER IF NOT EXISTS graph_revision_dns_host_delete AFTER DELETE ON dns_host
BEGIN UPDATE profile SET graph_revision = graph_revision + 1 WHERE id IN (OLD.profile_id); END;

CREATE TRIGGER IF NOT EXISTS graph_revision_profile AFTER UPDATE OF version, outbound_mode ON profile
WHEN NEW.version IS NOT OLD.version OR NEW.outbound_mode IS NOT OLD.outbound_mode
BEGIN UPDATE profile SET graph_revision = graph_revision + 1 WHERE id = NEW.id; END;
