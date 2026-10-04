use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    sync::Mutex,
};

use rusqlite::{params, Connection, OptionalExtension, Transaction};
use serde::{de::DeserializeOwned, Serialize};
use serde_json::Value;

use crate::core::dto::{
    DnsConfig, DnsHostEntry, DnsRule, DnsServer, LocalProxyConfig, ProfileMeta, Proxy, ProxyOrigin,
    ProxyPolicy, Rule, RuleSet, SubUserinfo, SyncSummary, TempRule, TrafficAnalytics, TrafficDelta,
    TrafficPoint, TrafficSummary, TrafficTopEntry, ZeytunCore, DEFAULT_PROFILE_ID,
};

#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    #[error(transparent)]
    Sqlite(#[from] rusqlite::Error),
    #[error("serialization error: {0}")]
    Serde(#[from] serde_json::Error),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("storage data error: {0}")]
    Data(String),
}

struct Migration {
    version: i64,
    name: &'static str,
    sql: &'static str,
}

const MIGRATIONS: &[Migration] = &[
    Migration {
        version: 1,
        name: "init",
        sql: include_str!("../../migrations/0001_init.sql"),
    },
    // NOTE: version 2 ("process_history") was applied by an earlier build and is
    // recorded in existing DBs even though it's not in this list — so this
    // migration must be version 3 to avoid being skipped as "already applied".
    Migration {
        version: 3,
        name: "traffic_analytics",
        sql: include_str!("../../migrations/0003_traffic_analytics.sql"),
    },
    Migration {
        version: 4,
        name: "profiles",
        sql: include_str!("../../migrations/0004_profiles.sql"),
    },
    // Rebuilds the per-profile tables with composite (profile_id, …) primary
    // keys and drops the legacy proxy.group_id NOT NULL column + grouping
    // tables. 0004 could only ALTER-ADD columns; SQLite can't change a PK or
    // drop a NOT NULL column in place, so the rebuild lives here.
    Migration {
        version: 5,
        name: "profile_rebuild",
        sql: include_str!("../../migrations/0005_profile_rebuild.sql"),
    },
    Migration {
        version: 6,
        name: "profile_auto_update",
        sql: include_str!("../../migrations/0006_profile_auto_update.sql"),
    },
    Migration {
        version: 7,
        name: "profile_icon",
        sql: include_str!("../../migrations/0007_profile_icon.sql"),
    },
    Migration {
        version: 8,
        name: "rulesets",
        sql: include_str!("../../migrations/0008_rulesets.sql"),
    },
    Migration {
        version: 9,
        name: "ruleset_download_policy",
        sql: include_str!("../../migrations/0009_ruleset_download_policy.sql"),
    },
    Migration {
        version: 10,
        name: "connection_ask",
        sql: include_str!("../../migrations/0010_connection_ask.sql"),
    },
    Migration {
        version: 11,
        name: "connection_ask_timeout_60s",
        sql: include_str!("../../migrations/0011_connection_ask_timeout_60s.sql"),
    },
    Migration {
        version: 12,
        name: "temp_route_rule",
        sql: include_str!("../../migrations/0012_temp_route_rule.sql"),
    },
    Migration {
        version: 13,
        name: "rule_enabled",
        sql: include_str!("../../migrations/0013_rule_enabled.sql"),
    },
    Migration {
        version: 14,
        name: "ruleset_name",
        sql: include_str!("../../migrations/0014_ruleset_name.sql"),
    },
    Migration {
        version: 15,
        name: "dns_rules",
        sql: include_str!("../../migrations/0015_dns_rules.sql"),
    },
    Migration {
        version: 16,
        name: "dns_hosts",
        sql: include_str!("../../migrations/0016_dns_hosts.sql"),
    },
    Migration {
        version: 17,
        name: "dns_server_name",
        sql: include_str!("../../migrations/0017_dns_server_name.sql"),
    },
    Migration {
        version: 18,
        name: "network_policy",
        sql: include_str!("../../migrations/0018_network_policy.sql"),
    },
    Migration {
        version: 19,
        name: "policy_strategy_weights",
        sql: include_str!("../../migrations/0019_policy_strategy_weights.sql"),
    },
];

pub trait Storage {
    fn path(&self) -> &Path;
    /// All profiles in the registry, ordered by `position`.
    fn list_profiles(&self) -> Result<Vec<ProfileMeta>, StorageError>;
    /// The active profile's id. Guaranteed non-empty after migration (the
    /// Default profile is always seeded active).
    fn active_profile_id(&self) -> Result<String, StorageError>;
    /// Load a profile's routing bundle (proxies/policies/rules/dns/rule_sets)
    /// plus the global local_proxy config and the profile's version/mode.
    fn load_profile(&self, id: &str) -> Result<ZeytunCore, StorageError>;
    /// Persist a profile's routing bundle (keyed on `profile.id`) and its
    /// version + outbound_mode. Leaves name/subscription_url/summary untouched.
    fn save_profile(&self, profile: &ZeytunCore) -> Result<(), StorageError>;
    fn create_profile(&self, meta: &ProfileMeta) -> Result<(), StorageError>;
    fn update_profile_meta(&self, meta: &ProfileMeta) -> Result<(), StorageError>;
    fn delete_profile(&self, id: &str) -> Result<(), StorageError>;
    /// Set `id` active and clear the flag on every other profile.
    fn set_active(&self, id: &str) -> Result<(), StorageError>;
    fn save_sync_summary(
        &self,
        id: &str,
        summary: &SyncSummary,
        unread: bool,
    ) -> Result<(), StorageError>;
    fn mark_summary_read(&self, id: &str) -> Result<(), StorageError>;
    /// Persist subscription metadata parsed from response headers after a sync:
    /// usage counters, expiry, and the server-advertised update interval. `None`
    /// fields are left unchanged.
    fn save_subscription_info(
        &self,
        id: &str,
        userinfo: Option<SubUserinfo>,
        update_interval_hours: Option<u32>,
    ) -> Result<(), StorageError>;
    /// Route chosen for each app-originated traffic class. Missing keys mean
    /// [`NetworkPolicy::Direct`] — callers must not assume every key is present.
    fn load_network_policies(
        &self,
    ) -> Result<HashMap<String, crate::core::network_policy::NetworkPolicy>, StorageError>;
    /// Upsert one traffic class's route.
    fn save_network_policy(
        &self,
        traffic: &str,
        policy: &crate::core::network_policy::NetworkPolicy,
    ) -> Result<(), StorageError>;
}

pub struct Db {
    path: PathBuf,
    /// Cached connection, opened and migrated on first use. Reused for every
    /// subsequent read/write instead of reopening (which re-ran migrations and
    /// pragmas on each `save_profile`).
    conn: Mutex<Option<Connection>>,
}

impl Db {
    pub fn new(path: PathBuf) -> Self {
        Self {
            path,
            conn: Mutex::new(None),
        }
    }

    /// Run `f` against the cached connection, opening (and migrating) it on
    /// first call. The connection is kept alive for the process lifetime.
    fn with_conn<T>(
        &self,
        f: impl FnOnce(&mut Connection) -> Result<T, StorageError>,
    ) -> Result<T, StorageError> {
        let mut guard = self
            .conn
            .lock()
            .map_err(|e| StorageError::Data(format!("lock poisoned: {e}")))?;
        if guard.is_none() {
            self.ensure_parent_dir()?;
            *guard = Some(self.open_connection()?);
        }
        let conn = guard.as_mut().expect("connection initialized above");
        f(conn)
    }

    fn read_profile(
        &self,
        conn: &Connection,
        id: &str,
    ) -> Result<Option<ZeytunCore>, StorageError> {
        let row = conn
            .query_row(
                "SELECT version, outbound_mode
                 FROM profile
                 WHERE id = ?1",
                params![id],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
            )
            .optional()?;

        let Some((version, outbound_mode)) = row else {
            return Ok(None);
        };

        let mut profile = ZeytunCore {
            id: id.to_string(),
            version,
            outbound_mode: enum_from_string(outbound_mode)?,
            local_proxy: self.read_local_proxy(conn)?,
            dns: self.read_dns(conn, id)?,
            rule_sets: self.read_rule_sets(conn, id)?,
            policies: self.read_policies(conn, id)?,
            proxies: self.read_proxies(conn, id)?,
            rules: self.read_rules(conn, id)?,
            temp_rules: self.read_temp_rules(conn, id)?,
            // Volatile session rules never survive a DB reload.
            session_rules: Vec::new(),
        };

        profile.normalize_policy_graph();

        Ok(Some(profile))
    }

    fn read_local_proxy(&self, conn: &Connection) -> Result<LocalProxyConfig, StorageError> {
        let row = conn
            .query_row(
                "SELECT mode, listen, socks_port, http_port, mixed_port, tun_name, tun_mtu, tun_auto_route, system_proxy,
                        connection_ask_enabled, connection_ask_timeout_ms, connection_ask_group_by, connection_ask_remember_default
                 FROM local_proxy_config
                 LIMIT 1",
                params![],
                |row| {
                    Ok((
                        row.get::<_, Option<String>>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, u16>(2)?,
                        row.get::<_, u16>(3)?,
                        row.get::<_, Option<u16>>(4)?,
                        row.get::<_, Option<String>>(5)?,
                        row.get::<_, Option<u32>>(6)?,
                        row.get::<_, Option<bool>>(7)?,
                        row.get::<_, Option<bool>>(8)?,
                        row.get::<_, bool>(9).unwrap_or(false),
                        row.get::<_, u32>(10).unwrap_or(60_000),
                        row.get::<_, String>(11).unwrap_or_else(|_| "process".to_string()),
                        row.get::<_, bool>(12).unwrap_or(true),
                    ))
                },
            )
            .optional()?;

        let Some((
            mode,
            listen,
            socks_port,
            http_port,
            mixed_port,
            tun_name,
            tun_mtu,
            tun_auto_route,
            system_proxy,
            ask_enabled,
            ask_timeout,
            ask_group,
            ask_remember,
        )) = row
        else {
            return Ok(ZeytunCore::default().local_proxy);
        };

        let connection_ask =
            if ask_enabled || ask_timeout != 60_000 || ask_group != "process" || !ask_remember {
                Some(crate::core::dto::ConnectionAskConfig {
                    enabled: ask_enabled,
                    timeout_ms: ask_timeout,
                    group_by: ask_group,
                    remember_default: ask_remember,
                })
            } else {
                None
            };

        Ok(LocalProxyConfig {
            mode: optional_enum_from_string(mode)?,
            listen,
            socks_port,
            http_port,
            mixed_port,
            tun_name,
            tun_mtu,
            tun_auto_route,
            system_proxy,
            log_level: None,
            connection_ask,
        })
    }

    fn read_dns(
        &self,
        conn: &Connection,
        profile_id: &str,
    ) -> Result<Option<DnsConfig>, StorageError> {
        let config = conn
            .query_row(
                "SELECT final_server, fake_ip FROM dns_config WHERE profile_id = ?1 LIMIT 1",
                params![profile_id],
                |row| {
                    Ok((
                        row.get::<_, Option<String>>(0)?,
                        row.get::<_, Option<bool>>(1)?,
                    ))
                },
            )
            .optional()?;

        let Some((final_server, fake_ip)) = config else {
            return Ok(None);
        };

        let mut stmt = conn.prepare(
            "SELECT tag, address, detour, name
                 FROM dns_server
                 WHERE profile_id = ?1
                 ORDER BY position ASC",
        )?;
        let servers = stmt
            .query_map(params![profile_id], |row| {
                Ok(DnsServer {
                    tag: row.get(0)?,
                    address: row.get(1)?,
                    detour: row.get(2)?,
                    name: row.get(3)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        let mut dns_rules_stmt = conn.prepare(
            "SELECT id, kind, value, target, comment, enabled
                 FROM dns_rule
                 WHERE profile_id = ?1
                 ORDER BY position ASC",
        )?;
        let dns_rules = dns_rules_stmt
            .query_map(params![profile_id], |row| {
                Ok(DnsRule {
                    id: row.get(0)?,
                    kind: row.get(1)?,
                    value: row.get(2)?,
                    target: row.get(3)?,
                    comment: row.get(4)?,
                    enabled: row.get(5)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        let mut dns_hosts_stmt = conn.prepare(
            "SELECT id, domain, address, enabled
                 FROM dns_host
                 WHERE profile_id = ?1
                 ORDER BY position ASC",
        )?;
        let dns_hosts = dns_hosts_stmt
            .query_map(params![profile_id], |row| {
                Ok(DnsHostEntry {
                    id: row.get(0)?,
                    domain: row.get(1)?,
                    address: row.get(2)?,
                    enabled: row.get(3)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(Some(DnsConfig {
            servers: (!servers.is_empty()).then_some(servers),
            final_server,
            fake_ip,
            rules: (!dns_rules.is_empty()).then_some(dns_rules),
            hosts: (!dns_hosts.is_empty()).then_some(dns_hosts),
        }))
    }

    fn read_rule_sets(
        &self,
        conn: &Connection,
        profile_id: &str,
    ) -> Result<Option<Vec<RuleSet>>, StorageError> {
        let mut stmt = conn.prepare(
            "SELECT id, tag, type, source, action, comment, download_policy, enabled, name
                 FROM rule_set
                 WHERE profile_id = ?1
                 ORDER BY position ASC",
        )?;
        let rule_sets = stmt
            .query_map(params![profile_id], |row| {
                Ok(RuleSet {
                    id: row.get(0)?,
                    tag: row.get(1)?,
                    kind: row.get(2)?,
                    source: row.get(3)?,
                    action: row.get(4)?,
                    comment: row.get(5)?,
                    download_policy: row.get(6)?,
                    enabled: row.get(7)?,
                    name: row.get(8)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok((!rule_sets.is_empty()).then_some(rule_sets))
    }

    fn read_policies(
        &self,
        conn: &Connection,
        profile_id: &str,
    ) -> Result<Vec<ProxyPolicy>, StorageError> {
        let mut stmt = conn.prepare(
            "SELECT tag, name, kind, selected_member_tag, test_url, interval_seconds, tolerance_ms,
                    strategy, weights_json
                 FROM policy
                 WHERE profile_id = ?1
                 ORDER BY position ASC",
        )?;
        let mut policies = stmt
            .query_map(params![profile_id], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<String>>(2)?,
                    row.get::<_, Option<String>>(3)?,
                    row.get::<_, Option<String>>(4)?,
                    row.get::<_, Option<u32>>(5)?,
                    row.get::<_, Option<u32>>(6)?,
                    row.get::<_, Option<String>>(7)?,
                    row.get::<_, Option<String>>(8)?,
                ))
            })?
            .map(|result| {
                let (
                    tag,
                    name,
                    kind,
                    selected_member_tag,
                    test_url,
                    interval_seconds,
                    tolerance_ms,
                    strategy,
                    weights_json,
                ) = result?;
                // Corrupt weights_json is a DB problem, not a JSON-API problem.
                let weights = weights_json
                    .as_deref()
                    .map(serde_json::from_str::<Vec<u32>>)
                    .transpose()
                    .map_err(|e| StorageError::Data(e.to_string()))?;
                Ok(ProxyPolicy {
                    tag,
                    name,
                    kind: optional_enum_from_string(kind)?,
                    members: None,
                    selected_member_tag,
                    test_url,
                    interval_seconds,
                    tolerance_ms,
                    strategy,
                    weights,
                })
            })
            .collect::<Result<Vec<_>, StorageError>>()?;

        let members = self.read_policy_members(conn, profile_id)?;
        for policy in &mut policies {
            policy.members = members
                .get(&policy.tag)
                .cloned()
                .filter(|items| !items.is_empty());
        }

        Ok(policies)
    }

    fn read_policy_members(
        &self,
        conn: &Connection,
        profile_id: &str,
    ) -> Result<HashMap<String, Vec<String>>, StorageError> {
        let mut stmt = conn.prepare(
            "SELECT policy_tag, member_tag
                 FROM policy_member
                 WHERE profile_id = ?1
                 ORDER BY position ASC",
        )?;
        let rows = stmt.query_map(params![profile_id], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;

        let mut members: HashMap<String, Vec<String>> = HashMap::new();
        for row in rows {
            let (policy_tag, member_tag) = row?;
            members.entry(policy_tag).or_default().push(member_tag);
        }
        Ok(members)
    }

    fn read_proxies(
        &self,
        conn: &Connection,
        profile_id: &str,
    ) -> Result<Vec<Proxy>, StorageError> {
        let mut stmt = conn.prepare(
            "SELECT tag, origin, title, protocol, link, enabled, transport, config_json
                 FROM proxy
                 WHERE profile_id = ?1
                 ORDER BY position ASC",
        )?;
        let proxies = stmt
            .query_map(params![profile_id], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, bool>(5)?,
                    row.get::<_, Option<String>>(6)?,
                    row.get::<_, Option<String>>(7)?,
                ))
            })?
            .map(|result| {
                let (tag, origin, title, protocol, link, enabled, transport, config_json) = result?;
                Ok(Proxy {
                    tag,
                    origin: enum_from_string(origin).unwrap_or(ProxyOrigin::Manual),
                    title,
                    protocol,
                    link,
                    enabled,
                    transport,
                    config: optional_json_from_string(config_json)?,
                })
            })
            .collect::<Result<Vec<_>, StorageError>>()?;
        Ok(proxies)
    }

    fn read_rules(&self, conn: &Connection, profile_id: &str) -> Result<Vec<Rule>, StorageError> {
        let mut stmt = conn.prepare(
            "SELECT id, kind, value, outbound, comment, rule_set, orphaned, enabled
                 FROM route_rule
                 WHERE profile_id = ?1
                 ORDER BY position ASC",
        )?;
        let rules = stmt
            .query_map(params![profile_id], |row| {
                Ok((
                    row.get::<_, u64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, Option<String>>(5)?,
                    row.get::<_, bool>(6)?,
                    row.get::<_, bool>(7)?,
                ))
            })?
            .map(|result| {
                let (id, kind, value, outbound, comment, rule_set, orphaned, enabled) = result?;
                Ok(Rule {
                    id,
                    kind: enum_from_string(kind)?,
                    value,
                    outbound,
                    comment,
                    rule_set,
                    orphaned,
                    enabled,
                })
            })
            .collect::<Result<Vec<_>, StorageError>>()?;
        Ok(rules)
    }

    fn read_temp_rules(
        &self,
        conn: &Connection,
        profile_id: &str,
    ) -> Result<Vec<TempRule>, StorageError> {
        let mut stmt = conn.prepare(
            "SELECT id, kind, value, outbound, comment, rule_set, orphaned, expires_at, enabled
                 FROM temp_route_rule
                 WHERE profile_id = ?1
                 ORDER BY position ASC",
        )?;
        let rules = stmt
            .query_map(params![profile_id], |row| {
                Ok((
                    row.get::<_, u64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, Option<String>>(5)?,
                    row.get::<_, bool>(6)?,
                    row.get::<_, i64>(7)?,
                    row.get::<_, bool>(8)?,
                ))
            })?
            .map(|result| {
                let (id, kind, value, outbound, comment, rule_set, orphaned, expires_at, enabled) =
                    result?;
                Ok(TempRule {
                    id,
                    kind: enum_from_string(kind)?,
                    value,
                    outbound,
                    comment,
                    rule_set,
                    orphaned,
                    expires_at,
                    enabled,
                    // Session rules are volatile app state — never persisted.
                    session: false,
                    ask_group_key: None,
                })
            })
            .collect::<Result<Vec<_>, StorageError>>()?;
        Ok(rules)
    }

    fn write_profile(
        &self,
        tx: &Transaction<'_>,
        profile: &ZeytunCore,
    ) -> Result<(), StorageError> {
        let id = if profile.id.is_empty() {
            DEFAULT_PROFILE_ID
        } else {
            profile.id.as_str()
        };
        self.clear_profile(tx, id)?;

        // Ensure the registry row exists, then update the routing-owned fields
        // (version + outbound_mode). name / subscription_url / summary are owned
        // by the profile-meta methods and left untouched here.
        tx.execute(
            "INSERT OR IGNORE INTO profile (id, name, is_active, outbound_mode, version)
             VALUES (?1, ?1, 0, ?2, ?3)",
            params![id, enum_to_string(&profile.outbound_mode)?, profile.version],
        )?;
        tx.execute(
            "UPDATE profile SET outbound_mode = ?2, version = ?3 WHERE id = ?1",
            params![id, enum_to_string(&profile.outbound_mode)?, profile.version],
        )?;

        // local_proxy_config is global (shared inbound settings), not scoped.
        self.write_local_proxy(tx, &profile.local_proxy)?;
        self.write_dns(tx, id, profile.dns.as_ref())?;
        self.write_rule_sets(tx, id, profile.rule_sets.as_deref().unwrap_or(&[]))?;
        self.write_policies(tx, id, &profile.policies)?;
        self.write_proxies(tx, id, &profile.proxies)?;
        self.write_rules(tx, id, &profile.rules)?;
        self.write_temp_rules(tx, id, &profile.temp_rules)?;

        Ok(())
    }

    /// Delete only `profile_id`'s routing rows. Global tables (setting,
    /// local_proxy_config) and the profile registry row are left intact.
    fn clear_profile(&self, tx: &Transaction<'_>, profile_id: &str) -> Result<(), StorageError> {
        for table in [
            "policy_member",
            "policy",
            "proxy",
            "route_rule",
            "temp_route_rule",
            "rule_set",
            "dns_rule",
            "dns_host",
            "dns_server",
            "dns_config",
        ] {
            tx.execute(
                &format!("DELETE FROM {table} WHERE profile_id = ?1"),
                params![profile_id],
            )?;
        }
        Ok(())
    }

    fn write_local_proxy(
        &self,
        tx: &Transaction<'_>,
        local_proxy: &LocalProxyConfig,
    ) -> Result<(), StorageError> {
        // Global singleton: replace the single row rather than accumulate.
        tx.execute("DELETE FROM local_proxy_config", [])?;
        let ask = local_proxy.connection_ask.as_ref();
        let ask_enabled = ask.map(|a| a.enabled).unwrap_or(false);
        let ask_timeout = ask.map(|a| a.timeout_ms).unwrap_or(60_000);
        let ask_group = ask
            .map(|a| a.group_by.as_str())
            .unwrap_or("process")
            .to_string();
        let ask_remember = ask.map(|a| a.remember_default).unwrap_or(true);
        tx.execute(
            "INSERT INTO local_proxy_config (
               mode, listen, socks_port, http_port, mixed_port, tun_name, tun_mtu, tun_auto_route, system_proxy,
               connection_ask_enabled, connection_ask_timeout_ms, connection_ask_group_by, connection_ask_remember_default
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
            params![
                optional_enum_to_string(local_proxy.mode.as_ref())?,
                local_proxy.listen,
                local_proxy.socks_port,
                local_proxy.http_port,
                local_proxy.mixed_port,
                local_proxy.tun_name,
                local_proxy.tun_mtu,
                local_proxy.tun_auto_route,
                local_proxy.system_proxy,
                ask_enabled,
                ask_timeout,
                ask_group,
                ask_remember,
            ],
        )?;
        Ok(())
    }

    fn write_dns(
        &self,
        tx: &Transaction<'_>,
        profile_id: &str,
        dns: Option<&DnsConfig>,
    ) -> Result<(), StorageError> {
        let Some(dns) = dns else {
            return Ok(());
        };

        tx.execute(
            "INSERT INTO dns_config (profile_id, final_server, fake_ip)
             VALUES (?1, ?2, ?3)",
            params![profile_id, dns.final_server, dns.fake_ip],
        )?;

        for (position, server) in dns.servers.as_deref().unwrap_or(&[]).iter().enumerate() {
            tx.execute(
                "INSERT INTO dns_server (profile_id, tag, address, detour, name, position)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    profile_id,
                    server.tag,
                    server.address,
                    server.detour,
                    server.name,
                    position as i64
                ],
            )?;
        }

        for (position, rule) in dns.rules.as_deref().unwrap_or(&[]).iter().enumerate() {
            tx.execute(
                "INSERT INTO dns_rule (profile_id, id, kind, value, target, comment, enabled, position)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    profile_id,
                    rule.id,
                    rule.kind,
                    rule.value,
                    rule.target,
                    rule.comment,
                    rule.enabled,
                    position as i64
                ],
            )?;
        }

        for (position, host) in dns.hosts.as_deref().unwrap_or(&[]).iter().enumerate() {
            tx.execute(
                "INSERT INTO dns_host (profile_id, id, domain, address, enabled, position)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    profile_id,
                    host.id,
                    host.domain,
                    host.address,
                    host.enabled,
                    position as i64
                ],
            )?;
        }
        Ok(())
    }

    fn write_rule_sets(
        &self,
        tx: &Transaction<'_>,
        profile_id: &str,
        rule_sets: &[RuleSet],
    ) -> Result<(), StorageError> {
        for (position, rule_set) in rule_sets.iter().enumerate() {
            tx.execute(
                "INSERT INTO rule_set (profile_id, id, tag, type, source, action, comment, download_policy, enabled, position, name)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
                params![
                    profile_id,
                    rule_set.id,
                    rule_set.tag,
                    rule_set.kind,
                    rule_set.source,
                    rule_set.action,
                    rule_set.comment,
                    rule_set.download_policy,
                    rule_set.enabled,
                    position as i64,
                    rule_set.name
                ],
            )?;
        }
        Ok(())
    }

    fn write_policies(
        &self,
        tx: &Transaction<'_>,
        profile_id: &str,
        policies: &[ProxyPolicy],
    ) -> Result<(), StorageError> {
        for (position, policy) in policies.iter().enumerate() {
            tx.execute(
                "INSERT INTO policy (
                   profile_id, tag, name, kind, selected_member_tag, test_url,
                   interval_seconds, tolerance_ms, position, strategy, weights_json
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
                params![
                    profile_id,
                    policy.tag,
                    policy.name,
                    optional_enum_to_string(policy.kind.as_ref())?,
                    policy.selected_member_tag,
                    policy.test_url,
                    policy.interval_seconds,
                    policy.tolerance_ms,
                    position as i64,
                    policy.strategy,
                    policy
                        .weights
                        .as_ref()
                        .map(serde_json::to_string)
                        .transpose()
                        .map_err(|e| rusqlite::Error::ToSqlConversionFailure(e.into()))?,
                ],
            )?;

            for (member_position, member) in
                policy.members.as_deref().unwrap_or(&[]).iter().enumerate()
            {
                tx.execute(
                    "INSERT INTO policy_member (profile_id, policy_tag, member_tag, position)
                     VALUES (?1, ?2, ?3, ?4)",
                    params![profile_id, policy.tag, member, member_position as i64],
                )?;
            }
        }
        Ok(())
    }

    fn write_proxies(
        &self,
        tx: &Transaction<'_>,
        profile_id: &str,
        proxies: &[Proxy],
    ) -> Result<(), StorageError> {
        for (position, proxy) in proxies.iter().enumerate() {
            tx.execute(
                "INSERT INTO proxy (
                   profile_id, tag, origin, title, protocol, link, enabled, transport, config_json, position
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                params![
                    profile_id,
                    proxy.tag,
                    enum_to_string(&proxy.origin)?,
                    proxy.title,
                    proxy.protocol,
                    proxy.link,
                    proxy.enabled,
                    proxy.transport,
                    optional_json_to_string(proxy.config.as_ref())?,
                    position as i64,
                ],
            )
            ?;
        }
        Ok(())
    }

    fn write_rules(
        &self,
        tx: &Transaction<'_>,
        profile_id: &str,
        rules: &[Rule],
    ) -> Result<(), StorageError> {
        for (position, rule) in rules.iter().enumerate() {
            tx.execute(
                "INSERT INTO route_rule (
                   profile_id, id, kind, value, outbound, enabled, comment, rule_set, orphaned, position
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                params![
                    profile_id,
                    rule.id,
                    enum_to_string(&rule.kind)?,
                    rule.value,
                    rule.outbound,
                    rule.enabled,
                    rule.comment,
                    rule.rule_set,
                    rule.orphaned,
                    position as i64,
                ],
            )?;
        }
        Ok(())
    }

    fn write_temp_rules(
        &self,
        tx: &Transaction<'_>,
        profile_id: &str,
        rules: &[TempRule],
    ) -> Result<(), StorageError> {
        for (position, rule) in rules.iter().enumerate() {
            tx.execute(
                "INSERT INTO temp_route_rule (
                   profile_id, id, kind, value, outbound, comment, rule_set, orphaned, expires_at, enabled, position
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
                params![
                    profile_id,
                    rule.id,
                    enum_to_string(&rule.kind)?,
                    rule.value,
                    rule.outbound,
                    rule.comment,
                    rule.rule_set,
                    rule.orphaned,
                    rule.expires_at,
                    rule.enabled,
                    position as i64,
                ],
            )?;
        }
        Ok(())
    }

    fn open_connection(&self) -> Result<Connection, StorageError> {
        let mut conn = Connection::open(&self.path)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        conn.busy_timeout(std::time::Duration::from_millis(5000))?;
        Self::run_migrations(&mut conn)?;
        Ok(conn)
    }

    fn run_migrations(conn: &mut Connection) -> Result<(), StorageError> {
        conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS schema_migration (
              version INTEGER PRIMARY KEY NOT NULL,
              name TEXT NOT NULL,
              applied_unix_ms INTEGER NOT NULL
            );
            ",
        )?;

        for migration in MIGRATIONS {
            let applied = conn
                .query_row(
                    "SELECT 1 FROM schema_migration WHERE version = ?1",
                    params![migration.version],
                    |_| Ok(()),
                )
                .optional()?
                .is_some();

            if applied {
                continue;
            }

            let tx = conn.transaction()?;
            tx.execute_batch(migration.sql)?;
            tx.execute(
                "INSERT INTO schema_migration (version, name, applied_unix_ms)
                 VALUES (?1, ?2, ?3)",
                params![migration.version, migration.name, unix_millis()],
            )?;
            tx.commit()?;
        }

        Ok(())
    }

    fn ensure_parent_dir(&self) -> Result<(), StorageError> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }
        Ok(())
    }

    /// Batch-upsert buffered traffic deltas in a single transaction. Existing
    /// (minute, domain, policy, process, is_proxy) rows accumulate; new ones insert.
    pub fn record_traffic_batch(&self, deltas: &[TrafficDelta]) -> Result<(), StorageError> {
        if deltas.is_empty() {
            return Ok(());
        }
        self.with_conn(|conn| {
            let tx = conn.transaction()?;
            {
                let mut stmt = tx.prepare(
                    "INSERT INTO traffic_analytics
                       (ts_minute, domain, policy, process, is_proxy, up_bytes, down_bytes)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
                     ON CONFLICT(ts_minute, domain, policy, process, is_proxy)
                     DO UPDATE SET up_bytes = up_bytes + excluded.up_bytes,
                                   down_bytes = down_bytes + excluded.down_bytes",
                )?;
                for d in deltas {
                    stmt.execute(params![
                        d.ts_minute,
                        d.domain,
                        d.policy,
                        d.process,
                        d.is_proxy as i64,
                        d.up_bytes,
                        d.down_bytes,
                    ])?;
                }
            }
            tx.commit()?;
            Ok(())
        })
    }

    /// Read the 24h (or arbitrary `since_ts`) analytics: per-minute series plus
    /// Top-10 consumers by domain / policy / process. `proxy_only` filters out
    /// direct traffic.
    pub fn query_traffic_analytics(
        &self,
        since_ts: i64,
        proxy_only: bool,
    ) -> Result<TrafficAnalytics, StorageError> {
        let filter = if proxy_only { " AND is_proxy = 1" } else { "" };
        self.with_conn(|conn| {
            let series = {
                let sql = format!(
                    "SELECT ts_minute, SUM(up_bytes), SUM(down_bytes)
                     FROM traffic_analytics
                     WHERE ts_minute >= ?1{filter}
                     GROUP BY ts_minute
                     ORDER BY ts_minute ASC"
                );
                let mut stmt = conn.prepare(&sql)?;
                let rows = stmt
                    .query_map(params![since_ts], |row| {
                        Ok(TrafficPoint {
                            ts: row.get(0)?,
                            up: row.get(1)?,
                            down: row.get(2)?,
                        })
                    })?
                    .collect::<Result<Vec<_>, _>>()?;
                rows
            };

            let top_domains = query_top(conn, "domain", since_ts, filter)?;
            let top_policies = query_top(conn, "policy", since_ts, filter)?;
            let top_processes = query_top(conn, "process", since_ts, filter)?;

            Ok(TrafficAnalytics {
                series,
                top_domains,
                top_policies,
                top_processes,
            })
        })
    }

    /// Direct vs proxy byte totals for `today` / `month`. `ts_minute` is stored
    /// as a UTC epoch, so the boundary uses `strftime('%s', …)` (also UTC) — they
    /// align. `period` only selects between two hardcoded boundary fragments.
    pub fn query_traffic_summary(&self, period: &str) -> Result<TrafficSummary, StorageError> {
        let boundary = match period {
            "month" => "strftime('%s', 'now', 'start of month')",
            _ => "strftime('%s', 'now', 'start of day')",
        };
        let sql = format!(
            "SELECT is_proxy, COALESCE(SUM(up_bytes + down_bytes), 0)
             FROM traffic_analytics
             WHERE ts_minute >= CAST({boundary} AS INTEGER)
             GROUP BY is_proxy"
        );
        self.with_conn(|conn| {
            let mut stmt = conn.prepare(&sql)?;
            let mut summary = TrafficSummary {
                direct_bytes: 0,
                proxy_bytes: 0,
            };
            let rows =
                stmt.query_map([], |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)))?;
            for row in rows {
                let (is_proxy, bytes) = row?;
                if is_proxy == 1 {
                    summary.proxy_bytes = bytes;
                } else {
                    summary.direct_bytes = bytes;
                }
            }
            Ok(summary)
        })
    }
}

/// Top-10 consumers grouped by a fixed column name (`column` is a hardcoded
/// identifier, never user input — no injection surface).
fn query_top(
    conn: &Connection,
    column: &str,
    since_ts: i64,
    filter: &str,
) -> Result<Vec<TrafficTopEntry>, StorageError> {
    let sql = format!(
        "SELECT {column} AS name, SUM(up_bytes) AS up, SUM(down_bytes) AS down
         FROM traffic_analytics
         WHERE ts_minute >= ?1{filter}
         GROUP BY {column}
         ORDER BY (up + down) DESC
         LIMIT 5"
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt
        .query_map(params![since_ts], |row| {
            Ok(TrafficTopEntry {
                name: row.get(0)?,
                up: row.get(1)?,
                down: row.get(2)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

impl Storage for Db {
    fn path(&self) -> &Path {
        &self.path
    }

    fn list_profiles(&self) -> Result<Vec<ProfileMeta>, StorageError> {
        self.with_conn(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, name, subscription_url, last_updated_unix_ms, is_active,
                        last_sync_summary, unread_summary, skip_auto_update,
                        update_interval_hours, sub_upload, sub_download, sub_total, sub_expire,
                        icon
                 FROM profile
                 ORDER BY position ASC, name ASC",
            )?;
            let rows = stmt
                .query_map(params![], |row| {
                    Ok((
                        ProfileMeta {
                            id: row.get(0)?,
                            name: row.get(1)?,
                            subscription_url: row.get(2)?,
                            last_updated_unix_ms: row.get(3)?,
                            is_active: row.get(4)?,
                            last_sync_summary: None, // filled below (needs JSON parse)
                            unread_summary: row.get(6)?,
                            skip_auto_update: row.get(7)?,
                            update_interval_hours: row.get(8)?,
                            sub_upload: row.get(9)?,
                            sub_download: row.get(10)?,
                            sub_total: row.get(11)?,
                            sub_expire: row.get(12)?,
                            icon: row.get(13)?,
                        },
                        row.get::<_, Option<String>>(5)?,
                    ))
                })?
                .collect::<Result<Vec<_>, _>>()?;

            rows.into_iter()
                .map(|(mut meta, summary_json)| {
                    meta.last_sync_summary = match summary_json {
                        Some(s) => Some(serde_json::from_str(&s)?),
                        None => None,
                    };
                    Ok(meta)
                })
                .collect::<Result<Vec<_>, StorageError>>()
        })
    }

    fn active_profile_id(&self) -> Result<String, StorageError> {
        self.with_conn(|conn| {
            let id = conn
                .query_row(
                    "SELECT id FROM profile WHERE is_active = 1 ORDER BY position ASC LIMIT 1",
                    params![],
                    |row| row.get::<_, String>(0),
                )
                .optional()?;
            Ok(id.unwrap_or_else(|| DEFAULT_PROFILE_ID.to_string()))
        })
    }

    fn load_profile(&self, id: &str) -> Result<ZeytunCore, StorageError> {
        let existing = self.with_conn(|conn| self.read_profile(conn, id))?;
        if let Some(profile) = existing {
            return Ok(profile);
        }

        // No such profile row yet — seed a default bundle under this id.
        let profile = ZeytunCore {
            id: id.to_string(),
            ..ZeytunCore::default()
        };
        self.save_profile(&profile)?;
        Ok(profile)
    }

    fn save_profile(&self, profile: &ZeytunCore) -> Result<(), StorageError> {
        self.with_conn(|conn| {
            let tx = conn.transaction()?;
            self.write_profile(&tx, profile)?;
            tx.commit()?;
            Ok(())
        })
    }

    fn create_profile(&self, meta: &ProfileMeta) -> Result<(), StorageError> {
        self.with_conn(|conn| {
            let tx = conn.transaction()?;
            let next_pos: i64 = tx
                .query_row(
                    "SELECT COALESCE(MAX(position), -1) + 1 FROM profile",
                    [],
                    |r| r.get(0),
                )
                .unwrap_or(0);
            tx.execute(
                "INSERT INTO profile (id, name, subscription_url, last_updated_unix_ms,
                                      is_active, outbound_mode, version, unread_summary,
                                      skip_auto_update, update_interval_hours, position, icon)
                 VALUES (?1, ?2, ?3, ?4, ?5, 'rule', '0.0.1', 0, ?6, ?7, ?8, ?9)",
                params![
                    meta.id,
                    meta.name,
                    meta.subscription_url,
                    meta.last_updated_unix_ms,
                    meta.is_active,
                    meta.skip_auto_update,
                    meta.update_interval_hours,
                    next_pos,
                    meta.icon,
                ],
            )?;
            tx.commit()?;
            Ok(())
        })
    }

    fn update_profile_meta(&self, meta: &ProfileMeta) -> Result<(), StorageError> {
        self.with_conn(|conn| {
            conn.execute(
                "UPDATE profile
                 SET name = ?2, subscription_url = ?3, last_updated_unix_ms = ?4,
                     skip_auto_update = ?5, update_interval_hours = ?6, icon = ?7
                 WHERE id = ?1",
                params![
                    meta.id,
                    meta.name,
                    meta.subscription_url,
                    meta.last_updated_unix_ms,
                    meta.skip_auto_update,
                    meta.update_interval_hours,
                    meta.icon,
                ],
            )?;
            Ok(())
        })
    }

    fn delete_profile(&self, id: &str) -> Result<(), StorageError> {
        self.with_conn(|conn| {
            let tx = conn.transaction()?;
            // Explicitly drop routing rows (FK ON DELETE CASCADE isn't declared
            // on the ALTER-added columns, so remove them by hand). Keep this
            // list identical to clear_profile's — both must cover every
            // profile-scoped table.
            for table in [
                "policy_member",
                "policy",
                "proxy",
                "route_rule",
                "temp_route_rule",
                "rule_set",
                "dns_rule",
                "dns_host",
                "dns_server",
                "dns_config",
            ] {
                tx.execute(
                    &format!("DELETE FROM {table} WHERE profile_id = ?1"),
                    params![id],
                )?;
            }
            tx.execute("DELETE FROM profile WHERE id = ?1", params![id])?;
            tx.commit()?;
            Ok(())
        })
    }

    fn set_active(&self, id: &str) -> Result<(), StorageError> {
        self.with_conn(|conn| {
            let tx = conn.transaction()?;
            tx.execute("UPDATE profile SET is_active = 0", [])?;
            tx.execute(
                "UPDATE profile SET is_active = 1 WHERE id = ?1",
                params![id],
            )?;
            tx.commit()?;
            Ok(())
        })
    }

    fn save_sync_summary(
        &self,
        id: &str,
        summary: &SyncSummary,
        unread: bool,
    ) -> Result<(), StorageError> {
        let json = serde_json::to_string(summary)?;
        self.with_conn(|conn| {
            conn.execute(
                "UPDATE profile
                 SET last_sync_summary = ?2, unread_summary = ?3, last_updated_unix_ms = ?4
                 WHERE id = ?1",
                params![id, json, unread, summary.at_unix_ms],
            )?;
            Ok(())
        })
    }

    fn mark_summary_read(&self, id: &str) -> Result<(), StorageError> {
        self.with_conn(|conn| {
            conn.execute(
                "UPDATE profile SET unread_summary = 0 WHERE id = ?1",
                params![id],
            )?;
            Ok(())
        })
    }

    fn save_subscription_info(
        &self,
        id: &str,
        userinfo: Option<SubUserinfo>,
        update_interval_hours: Option<u32>,
    ) -> Result<(), StorageError> {
        self.with_conn(|conn| {
            if let Some(u) = userinfo {
                conn.execute(
                    "UPDATE profile
                     SET sub_upload = ?2, sub_download = ?3, sub_total = ?4, sub_expire = ?5
                     WHERE id = ?1",
                    params![id, u.upload, u.download, u.total, u.expire],
                )?;
            }
            if let Some(hours) = update_interval_hours {
                conn.execute(
                    "UPDATE profile SET update_interval_hours = ?2 WHERE id = ?1",
                    params![id, hours],
                )?;
            }
            Ok(())
        })
    }

    fn load_network_policies(
        &self,
    ) -> Result<HashMap<String, crate::core::network_policy::NetworkPolicy>, StorageError> {
        self.with_conn(|conn| {
            let mut stmt = conn.prepare("SELECT traffic, policy FROM network_policy")?;
            let rows = stmt
                .query_map([], |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
                })?
                .collect::<Result<Vec<_>, _>>()?;

            let mut out = HashMap::new();
            for (traffic, raw) in rows {
                // A row we can't parse is a row written by a future build. Skip
                // it rather than failing the whole read — the caller falls back
                // to Direct, which is the documented default.
                match serde_json::from_str(&raw) {
                    Ok(policy) => {
                        out.insert(traffic, policy);
                    }
                    Err(e) => {
                        println!("[Storage] unreadable network_policy for `{traffic}`: {e}");
                    }
                }
            }
            Ok(out)
        })
    }

    fn save_network_policy(
        &self,
        traffic: &str,
        policy: &crate::core::network_policy::NetworkPolicy,
    ) -> Result<(), StorageError> {
        let encoded = serde_json::to_string(policy)?;
        self.with_conn(|conn| {
            conn.execute(
                "INSERT INTO network_policy (traffic, policy) VALUES (?1, ?2)
                 ON CONFLICT(traffic) DO UPDATE SET policy = excluded.policy",
                params![traffic, encoded],
            )?;
            Ok(())
        })
    }
}

fn enum_to_string<T: Serialize>(value: &T) -> Result<String, StorageError> {
    serde_json::to_value(value)?
        .as_str()
        .map(|s| s.trim_matches('"').to_owned())
        .ok_or_else(|| StorageError::Data("enum did not serialize to a string".to_string()))
}

fn optional_enum_to_string<T: Serialize>(
    value: Option<&T>,
) -> Result<Option<String>, StorageError> {
    value.map(enum_to_string).transpose()
}

fn enum_from_string<T: DeserializeOwned>(value: String) -> Result<T, StorageError> {
    Ok(serde_json::from_value(Value::String(value))?)
}

fn optional_enum_from_string<T: DeserializeOwned>(
    value: Option<String>,
) -> Result<Option<T>, StorageError> {
    value.map(enum_from_string).transpose()
}

fn json_to_string(value: &Value) -> Result<String, StorageError> {
    Ok(serde_json::to_string(value)?)
}

fn optional_json_to_string(value: Option<&Value>) -> Result<Option<String>, StorageError> {
    value.map(json_to_string).transpose()
}

fn json_from_string(value: String) -> Result<Value, StorageError> {
    Ok(serde_json::from_str(&value)?)
}

fn optional_json_from_string(value: Option<String>) -> Result<Option<Value>, StorageError> {
    value.map(json_from_string).transpose()
}

fn unix_millis() -> i64 {
    crate::core::constants::unix_millis()
}

#[cfg(test)]
use crate::core::dto::ProxyPolicyType;

#[cfg(test)]
mod tests {
    use super::*;

    fn dns_rule(id: &str, value: &str, target: &str, enabled: bool) -> DnsRule {
        DnsRule {
            id: id.to_string(),
            kind: "domain".to_string(),
            value: value.to_string(),
            target: target.to_string(),
            comment: None,
            enabled,
        }
    }

    #[test]
    fn replacing_dns_rules_persists_order_edits_and_enabled_state() {
        let test_dir =
            std::env::temp_dir().join(format!("castle-dns-rule-storage-{}", uuid::Uuid::new_v4()));
        let db = Db::new(test_dir.join("zeytun.db"));
        let mut profile = ZeytunCore {
            dns: Some(DnsConfig {
                servers: Some(vec![DnsServer {
                    tag: "primary".to_string(),
                    address: "1.1.1.1".to_string(),
                    detour: None,
                    name: "Primary DNS".to_string(),
                }]),
                final_server: Some("primary".to_string()),
                fake_ip: None,
                rules: Some(vec![
                    dns_rule("first", "old.example", "primary", true),
                    dns_rule("second", "second.example", "primary", true),
                ]),
                hosts: None,
            }),
            ..ZeytunCore::default()
        };
        db.save_profile(&profile).expect("save initial DNS rules");

        profile.dns.as_mut().expect("DNS config").rules = Some(vec![
            dns_rule("second", "edited.example", "primary", false),
            dns_rule("third", "new.example", "block", true),
        ]);
        db.save_profile(&profile)
            .expect("replace persisted DNS rules without primary-key conflicts");

        let loaded = db.load_profile(&profile.id).expect("reload profile");
        let rules = loaded
            .dns
            .and_then(|dns| dns.rules)
            .expect("persisted DNS rules");
        assert_eq!(
            rules
                .iter()
                .map(|rule| rule.id.as_str())
                .collect::<Vec<_>>(),
            vec!["second", "third"]
        );
        assert_eq!(rules[0].value, "edited.example");
        assert!(!rules[0].enabled);
        assert_eq!(rules[1].target, "block");

        drop(db);
        let _ = std::fs::remove_dir_all(test_dir);
    }

    #[test]
    fn replacing_dns_hosts_roundtrips_add_edit_toggle_delete_and_order() {
        let test_dir =
            std::env::temp_dir().join(format!("castle-dns-host-storage-{}", uuid::Uuid::new_v4()));
        let db = Db::new(test_dir.join("zeytun.db"));
        let mut profile = ZeytunCore {
            dns: Some(DnsConfig {
                hosts: Some(vec![
                    DnsHostEntry {
                        id: "first".to_string(),
                        domain: "one.example".to_string(),
                        address: "192.0.2.1".to_string(),
                        enabled: true,
                    },
                    DnsHostEntry {
                        id: "second".to_string(),
                        domain: "two.example".to_string(),
                        address: "2001:db8::2".to_string(),
                        enabled: true,
                    },
                ]),
                ..Default::default()
            }),
            ..ZeytunCore::default()
        };
        db.save_profile(&profile).expect("save initial DNS hosts");

        profile.dns.as_mut().expect("DNS config").hosts = Some(vec![
            DnsHostEntry {
                id: "second".to_string(),
                domain: "edited.example".to_string(),
                address: "2001:db8::20".to_string(),
                enabled: false,
            },
            DnsHostEntry {
                id: "third".to_string(),
                domain: "three.example".to_string(),
                address: "192.0.2.3".to_string(),
                enabled: true,
            },
        ]);
        db.save_profile(&profile)
            .expect("replace persisted DNS hosts without primary-key conflicts");

        let loaded = db.load_profile(&profile.id).expect("reload profile");
        let hosts = loaded
            .dns
            .and_then(|dns| dns.hosts)
            .expect("persisted DNS hosts");
        assert_eq!(
            hosts
                .iter()
                .map(|host| host.id.as_str())
                .collect::<Vec<_>>(),
            vec!["second", "third"]
        );
        assert_eq!(hosts[0].domain, "edited.example");
        assert_eq!(hosts[0].address, "2001:db8::20");
        assert!(!hosts[0].enabled);
        assert_eq!(hosts[1].address, "192.0.2.3");

        drop(db);
        let _ = std::fs::remove_dir_all(test_dir);
    }
}

#[test]
fn proxies_roundtrip_through_fresh_connection() {
    // The config blob is what link_parser produces and the compiler
    // consumes; losing it means a saved proxy has no server config.
    let test_dir = std::env::temp_dir().join(format!("castle-proxy-rt-{}", uuid::Uuid::new_v4()));
    let db = Db::new(test_dir.join("zeytun.db"));

    let profile = ZeytunCore {
        proxies: vec![
            Proxy {
                tag: "p1".into(),
                origin: ProxyOrigin::Manual,
                title: "My VLESS".into(),
                protocol: "vless".into(),
                link: "vless://uuid@host:443".into(),
                enabled: true,
                transport: Some("ws".into()),
                config: Some(serde_json::json!({
                    "type": "vless", "uuid": "5e3daa0d-9c9a-4f0f-9a1d-6ce35eeb3fc3",
                    "tag": "p1", "address": "host", "port": 443,
                    "tls": { "enabled": true, "sni": "host" },
                })),
            },
            Proxy {
                tag: "p2".into(),
                origin: ProxyOrigin::Subscription,
                title: "Sub SS".into(),
                protocol: "shadowsocks".into(),
                link: "ss://...".into(),
                enabled: false,
                transport: None,
                config: None,
            },
        ],
        ..ZeytunCore::default()
    };
    db.save_profile(&profile).expect("save proxies");

    // A fresh Db on the same path is the real test — nothing cached.
    let db2 = Db::new(test_dir.join("zeytun.db"));
    let loaded = db2.load_profile(&profile.id).expect("reload");

    assert_eq!(loaded.proxies.len(), 2);
    let p1 = &loaded.proxies[0];
    assert_eq!(p1.tag, "p1");
    assert_eq!(p1.title, "My VLESS");
    assert_eq!(p1.protocol, "vless");
    assert_eq!(p1.transport.as_deref(), Some("ws"));
    assert!(p1.enabled);
    assert_eq!(p1.origin, ProxyOrigin::Manual);
    let cfg = p1.config.as_ref().expect("config blob survived");
    assert_eq!(cfg["uuid"], "5e3daa0d-9c9a-4f0f-9a1d-6ce35eeb3fc3");
    assert_eq!(cfg["tls"]["sni"], "host");

    let p2 = &loaded.proxies[1];
    assert!(!p2.enabled);
    assert!(p2.config.is_none(), "config: None stays absent");

    drop(db);
    let _ = std::fs::remove_dir_all(test_dir);
}

#[test]
fn policies_roundtrip_members_weights_and_strategy() {
    let test_dir = std::env::temp_dir().join(format!("castle-policy-rt-{}", uuid::Uuid::new_v4()));
    let db = Db::new(test_dir.join("zeytun.db"));

    // normalize_policy_graph drops members that point at nothing real, so the
    // group's members must reference actual proxies to survive the round-trip.
    let mk_proxy = |tag: &str| Proxy {
        tag: tag.to_string(),
        origin: ProxyOrigin::Manual,
        title: format!("{tag} node"),
        protocol: "socks".to_string(),
        link: format!("socks5://{tag}.example:1080"),
        enabled: true,
        transport: None,
        config: None,
    };
    let profile = ZeytunCore {
        proxies: vec![mk_proxy("p1"), mk_proxy("p2"), mk_proxy("p3")],
        policies: vec![ProxyPolicy {
            tag: "grp".into(),
            name: "My Group".into(),
            kind: Some(ProxyPolicyType::Balancer),
            members: Some(vec!["p1".into(), "p2".into(), "p3".into()]),
            selected_member_tag: Some("p2".into()),
            test_url: Some("https://cp.cloudflare.com/generate_204".into()),
            interval_seconds: Some(300),
            tolerance_ms: Some(50),
            strategy: Some("ping".into()),
            weights: Some(vec![10, 20, 30]),
        }],
        ..ZeytunCore::default()
    };
    db.save_profile(&profile).expect("save policy");

    let db2 = Db::new(test_dir.join("zeytun.db"));
    let loaded = db2.load_profile(&profile.id).expect("reload");

    // ZeytunCore::default() injects a root policy; find ours by tag.
    let pol = loaded
        .policies
        .iter()
        .find(|p| p.tag == "grp")
        .expect("our policy survived");
    assert_eq!(pol.tag, "grp");
    assert_eq!(pol.strategy.as_deref(), Some("ping"));
    assert_eq!(
        pol.members.as_deref().unwrap_or(&[]),
        &["p1", "p2", "p3"],
        "member order must survive"
    );
    assert_eq!(pol.weights.as_deref(), Some(&[10u32, 20, 30][..]));
    assert_eq!(pol.selected_member_tag.as_deref(), Some("p2"));
    assert_eq!(pol.interval_seconds, Some(300));
    assert_eq!(pol.tolerance_ms, Some(50));

    drop(db);
    let _ = std::fs::remove_dir_all(test_dir);
}

#[test]
fn sync_summary_roundtrips_and_mark_read_clears_only_the_flag() {
    let test_dir = std::env::temp_dir().join(format!("castle-sync-rt-{}", uuid::Uuid::new_v4()));
    let db = Db::new(test_dir.join("zeytun.db"));

    let meta = ProfileMeta {
        id: "summary-prof".to_string(),
        name: "Summary Test".to_string(),
        subscription_url: None,
        last_updated_unix_ms: None,
        is_active: false,
        last_sync_summary: None,
        unread_summary: false,
        skip_auto_update: false,
        update_interval_hours: 12,
        sub_upload: None,
        sub_download: None,
        sub_total: None,
        sub_expire: None,
        icon: None,
    };
    db.create_profile(&meta).expect("create profile row");

    // save_sync_summary is what a real subscription sync writes.
    let summary = SyncSummary {
        proxies_added: 3,
        proxies_removed: 1,
        proxies_kept: 5,
        rules_orphaned: 2,
        at_unix_ms: 1_700_000_000_000,
        errors: vec!["bad link".to_string()],
    };
    db.save_sync_summary("summary-prof", &summary, true)
        .expect("save summary");

    // A fresh Db on the same path: nothing is cached in memory.
    let db2 = Db::new(test_dir.join("zeytun.db"));
    let loaded = db2
        .list_profiles()
        .expect("list profiles")
        .into_iter()
        .find(|m| m.id == "summary-prof")
        .expect("profile found");

    let s = loaded.last_sync_summary.as_ref().expect("summary survived");
    assert_eq!(s.proxies_added, 3);
    assert_eq!(s.proxies_kept, 5);
    assert_eq!(s.rules_orphaned, 2);
    assert_eq!(s.at_unix_ms, 1_700_000_000_000);
    assert_eq!(s.errors, vec!["bad link"]);
    assert!(loaded.unread_summary, "the unread flag is its own column");

    // mark_summary_read clears the flag but keeps the body, so the user can
    // still review what the last sync changed.
    db2.mark_summary_read("summary-prof").expect("mark read");
    let after = db2
        .list_profiles()
        .expect("list profiles")
        .into_iter()
        .find(|m| m.id == "summary-prof")
        .expect("profile found");
    assert!(!after.unread_summary, "flag cleared");
    assert!(after.last_sync_summary.is_some(), "summary body preserved");

    drop(db);
    let _ = std::fs::remove_dir_all(test_dir);
}
