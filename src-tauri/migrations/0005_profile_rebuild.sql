-- 0004 added `profile_id` / `origin` / `orphaned` columns via ALTER, but ALTER
-- cannot change a table's PRIMARY KEY or drop the legacy `proxy.group_id NOT NULL`
-- column. That left two latent breakages:
--   1. proxy.group_id is still NOT NULL with no default, so save_profile (which
--      no longer supplies it) fails immediately: "NOT NULL constraint failed".
--   2. Single-column PKs (policy.tag, route_rule.id, rule_set.id, dns_server.tag,
--      policy_member(policy_tag,member_tag)) collide the instant a second profile
--      writes the same tag / final-rule id=0.
--
-- Fix: rebuild every per-profile table with a composite PK that includes
-- profile_id and a FK to profile(id) ON DELETE CASCADE, dropping group_id. This
-- is the standard SQLite table-rebuild (rename → create → copy → drop). It runs
-- from the post-0004 schema, so profile_id / origin / orphaned already exist on
-- the source rows. Existing data is preserved (profile_id defaults to 'default').
--
-- proxy_group must stay alive until AFTER proxy is rebuilt: the old proxy table
-- has a FK to it with ON DELETE CASCADE, so dropping proxy_group first would
-- implicitly delete every proxy row. It is dropped last, once nothing references
-- it.

-- ----- proxy ----------------------------------------------------------------
ALTER TABLE proxy RENAME TO proxy_legacy;

CREATE TABLE proxy (
  profile_id  TEXT    NOT NULL,
  tag         TEXT    NOT NULL,
  origin      TEXT    NOT NULL DEFAULT 'manual',
  title       TEXT    NOT NULL,
  protocol    TEXT    NOT NULL,
  link        TEXT    NOT NULL,
  enabled     INTEGER NOT NULL,
  transport   TEXT,
  config_json TEXT,
  position    INTEGER NOT NULL,
  PRIMARY KEY(profile_id, tag),
  FOREIGN KEY(profile_id) REFERENCES profile(id) ON DELETE CASCADE
);

INSERT INTO proxy (profile_id, tag, origin, title, protocol, link, enabled, transport, config_json, position)
SELECT COALESCE(profile_id, 'default'),
       tag,
       COALESCE(origin, 'manual'),
       title, protocol, link, enabled, transport, config_json, position
FROM proxy_legacy;

DROP TABLE proxy_legacy;

-- ----- policy ---------------------------------------------------------------
ALTER TABLE policy RENAME TO policy_legacy;

CREATE TABLE policy (
  profile_id          TEXT    NOT NULL,
  tag                 TEXT    NOT NULL,
  name                TEXT    NOT NULL,
  kind                TEXT,
  selected_member_tag TEXT,
  test_url            TEXT,
  interval_seconds    INTEGER,
  tolerance_ms        INTEGER,
  position            INTEGER NOT NULL,
  PRIMARY KEY(profile_id, tag),
  FOREIGN KEY(profile_id) REFERENCES profile(id) ON DELETE CASCADE
);

INSERT INTO policy (profile_id, tag, name, kind, selected_member_tag, test_url, interval_seconds, tolerance_ms, position)
SELECT COALESCE(profile_id, 'default'),
       tag, name, kind, selected_member_tag, test_url, interval_seconds, tolerance_ms, position
FROM policy_legacy;

DROP TABLE policy_legacy;

-- ----- policy_member --------------------------------------------------------
ALTER TABLE policy_member RENAME TO policy_member_legacy;

CREATE TABLE policy_member (
  profile_id  TEXT    NOT NULL,
  policy_tag  TEXT    NOT NULL,
  member_tag  TEXT    NOT NULL,
  position    INTEGER NOT NULL,
  PRIMARY KEY(profile_id, policy_tag, member_tag),
  FOREIGN KEY(profile_id) REFERENCES profile(id) ON DELETE CASCADE
);

INSERT INTO policy_member (profile_id, policy_tag, member_tag, position)
SELECT COALESCE(profile_id, 'default'), policy_tag, member_tag, position
FROM policy_member_legacy;

DROP TABLE policy_member_legacy;

-- ----- route_rule -----------------------------------------------------------
ALTER TABLE route_rule RENAME TO route_rule_legacy;

CREATE TABLE route_rule (
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

INSERT INTO route_rule (profile_id, id, kind, value, outbound, enabled, comment, rule_set, orphaned, position)
SELECT COALESCE(profile_id, 'default'),
       id, kind, value, outbound, enabled, comment, rule_set,
       COALESCE(orphaned, 0), position
FROM route_rule_legacy;

DROP TABLE route_rule_legacy;

-- ----- rule_set -------------------------------------------------------------
ALTER TABLE rule_set RENAME TO rule_set_legacy;

CREATE TABLE rule_set (
  profile_id TEXT    NOT NULL,
  id         TEXT    NOT NULL,
  source     TEXT    NOT NULL,
  format     TEXT,
  position   INTEGER NOT NULL,
  PRIMARY KEY(profile_id, id),
  FOREIGN KEY(profile_id) REFERENCES profile(id) ON DELETE CASCADE
);

INSERT INTO rule_set (profile_id, id, source, format, position)
SELECT COALESCE(profile_id, 'default'), id, source, format, position
FROM rule_set_legacy;

DROP TABLE rule_set_legacy;

-- ----- dns_config (one row per profile) -------------------------------------
ALTER TABLE dns_config RENAME TO dns_config_legacy;

CREATE TABLE dns_config (
  profile_id   TEXT NOT NULL,
  final_server TEXT,
  fake_ip      INTEGER,
  PRIMARY KEY(profile_id),
  FOREIGN KEY(profile_id) REFERENCES profile(id) ON DELETE CASCADE
);

INSERT INTO dns_config (profile_id, final_server, fake_ip)
SELECT COALESCE(profile_id, 'default'), final_server, fake_ip
FROM dns_config_legacy;

DROP TABLE dns_config_legacy;

-- ----- dns_server -----------------------------------------------------------
ALTER TABLE dns_server RENAME TO dns_server_legacy;

CREATE TABLE dns_server (
  profile_id TEXT    NOT NULL,
  tag        TEXT    NOT NULL,
  address    TEXT    NOT NULL,
  detour     TEXT,
  position   INTEGER NOT NULL,
  PRIMARY KEY(profile_id, tag),
  FOREIGN KEY(profile_id) REFERENCES profile(id) ON DELETE CASCADE
);

INSERT INTO dns_server (profile_id, tag, address, detour, position)
SELECT COALESCE(profile_id, 'default'), tag, address, detour, position
FROM dns_server_legacy;

DROP TABLE dns_server_legacy;

-- ----- drop the legacy grouping tables (now unreferenced) -------------------
DROP TABLE IF EXISTS group_item;
DROP TABLE IF EXISTS proxy_group;

-- ----- indexes --------------------------------------------------------------
CREATE INDEX IF NOT EXISTS idx_proxy_profile        ON proxy(profile_id, position);
CREATE INDEX IF NOT EXISTS idx_policy_profile       ON policy(profile_id, position);
CREATE INDEX IF NOT EXISTS idx_policy_member_prof   ON policy_member(profile_id, policy_tag, position);
CREATE INDEX IF NOT EXISTS idx_route_rule_profile   ON route_rule(profile_id, position);
CREATE INDEX IF NOT EXISTS idx_rule_set_profile     ON rule_set(profile_id, position);
CREATE INDEX IF NOT EXISTS idx_dns_server_profile   ON dns_server(profile_id, position);
