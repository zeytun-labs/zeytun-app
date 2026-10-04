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
  log_level TEXT
);


CREATE TABLE IF NOT EXISTS dns_config (
  final_server TEXT,
  fake_ip INTEGER
);

CREATE TABLE IF NOT EXISTS dns_server (
  tag TEXT NOT NULL,
  address TEXT NOT NULL,
  detour TEXT,
  position INTEGER NOT NULL,
  PRIMARY KEY(tag)
);

CREATE TABLE IF NOT EXISTS rule_set (
  id TEXT NOT NULL,
  source TEXT NOT NULL,
  format TEXT,
  position INTEGER NOT NULL,
  PRIMARY KEY(id)
);

CREATE TABLE IF NOT EXISTS proxy_group (
  id TEXT NOT NULL,
  name TEXT NOT NULL,
  url TEXT,
  auto_update INTEGER,
  update_interval_secs INTEGER,
  last_updated_unix_ms INTEGER,
  position INTEGER NOT NULL,
  PRIMARY KEY(id)
);

CREATE TABLE IF NOT EXISTS policy (
  tag TEXT NOT NULL,
  name TEXT NOT NULL,
  kind TEXT,
  selected_member_tag TEXT,
  test_url TEXT,
  interval_seconds INTEGER,
  tolerance_ms INTEGER,
  position INTEGER NOT NULL,
  PRIMARY KEY(tag)
);

CREATE TABLE IF NOT EXISTS policy_member (
  policy_tag TEXT NOT NULL,
  member_tag TEXT NOT NULL,
  position INTEGER NOT NULL,
  PRIMARY KEY(policy_tag, member_tag)
);

CREATE TABLE IF NOT EXISTS proxy (
  tag TEXT NOT NULL,
  group_id TEXT NOT NULL,
  title TEXT NOT NULL,
  protocol TEXT NOT NULL,
  link TEXT NOT NULL,
  enabled INTEGER NOT NULL,
  transport TEXT,
  config_json TEXT,
  position INTEGER NOT NULL,
  PRIMARY KEY(tag),
  FOREIGN KEY(group_id) REFERENCES proxy_group(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS group_item (
  group_id TEXT NOT NULL,
  item_type TEXT NOT NULL CHECK(item_type IN ('proxy', 'policy')),
  item_tag TEXT NOT NULL,
  position INTEGER NOT NULL,
  PRIMARY KEY(group_id, item_type, item_tag),
  FOREIGN KEY(group_id) REFERENCES proxy_group(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS route_rule (
  id INTEGER NOT NULL,
  kind TEXT NOT NULL,
  value TEXT NOT NULL,
  outbound TEXT NOT NULL,
  enabled INTEGER NOT NULL,
  comment TEXT NOT NULL,
  rule_set TEXT,
  position INTEGER NOT NULL,
  PRIMARY KEY(id)
);

CREATE INDEX IF NOT EXISTS idx_group_item_group_position
  ON group_item(group_id, position);

CREATE INDEX IF NOT EXISTS idx_proxy_group_position
  ON proxy(group_id, position);

CREATE INDEX IF NOT EXISTS idx_policy_member_policy_position
  ON policy_member(policy_tag, position);
