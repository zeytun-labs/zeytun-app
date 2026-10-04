export type OutboundMode = "direct" | "global" | "rule";

export type RuleType =
  | "FINAL"
  | "DOMAIN"
  | "DOMAIN-SUFFIX"
  | "DOMAIN-KEYWORD"
  | "DOMAIN-REGEX"
  | "IP-CIDR"
  | "IN-PORT"
  | "DEST-PORT"
  | "GEOIP"
  | "GEOSITE"
  | "PROCESS-NAME"
  | "PROCESS-PATH"
  | "PROCESS-PATH-REGEX"
  | "PROTOCOL";

/// Result of `core_resolve_process`: display name for a process-style rule
/// value plus the cached icon PNG path (load via `convertFileSrc`).
export type ResolvedProcess = {
  name: string;
  iconPath?: string | null;
};

export type RuntimePhase =
  "idle" | "starting" | "running" | "stopping" | "error";

export type CoreEventType = "info" | "warning" | "error";

export type CoreEvent = {
  id: string;
  timestamp: number;
  type: CoreEventType;
  title: string;
  message: string;
};

/// Result of `core_run_diagnostics`: raw numbers for the dashboard card plus a
/// preformatted plaintext report for the diagnostics sheet.
export type DiagnosticsResult = {
  directMs: number | null;
  routerMs: number | null;
  dnsMs: number | null;
  proxyMs: number | null;
  proxyTested: boolean;
  report: string;
};

export type InboundMode = "mixed" | "tun";
export type ProxyEngine = "zeytun-core" | "unsupported";
export type ProxyPolicyType = "selector" | "urltest" | "balancer";

export type BalancerStrategy =
  | "round-robin"
  | "consistent-hashing"
  | "sticky-sessions"
  | "failover"
  | "weighted"
  | "least-connections";

export type LocalProxyConfig = {
  mode?: InboundMode;
  listen: string;
  socks_port: number;
  http_port: number;
  mixed_port?: number;
  tun_name?: string;
  tun_mtu?: number;
  tun_auto_route?: boolean;
  system_proxy?: boolean;
  connection_ask?: ConnectionAskConfig | null;
};

export type ConnectionAskConfig = {
  enabled: boolean;
  timeout_ms: number;
  group_by: string; // process | process_dest
  remember_default: boolean;
};

export type ProxyPolicy = {
  tag: string;
  name: string;
  kind?: ProxyPolicyType;
  members?: string[];
  selected_member_tag?: string | null;
  test_url?: string | null;
  interval_seconds?: number | null;
  tolerance_ms?: number | null;

  // balancer specific
  strategy?: BalancerStrategy | null;
  delay_acceptable_ratio?: number | null;
  ttl?: string | null;
  max_retry?: number | null;
  weights?: number[] | null;
};

export type ProxyOrigin = "subscription" | "manual";

export type Proxy = {
  tag: string;
  origin: ProxyOrigin;
  title: string;
  protocol: string;
  link: string;
  enabled: boolean;
  transport?: string | null;
  config?: ProxyServerConfig | null;
};

export type ProxyProtocol =
  | "vless"
  | "vmess"
  | "trojan"
  | "shadowsocks"
  | "hysteria2"
  | "tuic"
  | "socks"
  | "http"
  | "chain";

export type TlsConfig = {
  allow_insecure: boolean;
  certificate?: string | null;
  sni?: string | null;
  alpn?: string[] | null;
  fragment: boolean;
  fallback_delay?: string | null;
  record_fragment: boolean;
  fingerprint?: string | null;
  reality_pbk?: string | null;
  reality_sid?: string | null;
};

export type ProxyServerConfig = {
  tag: string;
  name: string;
  address: string;
  port: number;
  type: ProxyProtocol;
  // VLESS
  uuid?: string;
  flow?: string | null;
  packet_encoding?: string | null;
  network?: string;
  transport?: {
    path?: string | null;
    host?: string | null;
    mode?: string | null;
    service_name?: string | null;
    download?: {
      address?: string | null;
      port?: number | null;
      path?: string | null;
      host?: string | null;
      detour?: string | null;
      security?: string | null;
      sni?: string | null;
      allow_insecure?: boolean | null;
      alpn?: string[] | null;
      fingerprint?: string | null;
      reality_pbk?: string | null;
      reality_sid?: string | null;
    } | null;
  } | null;
  tls?: TlsConfig | null;
  mux?: boolean | null;
  tcp_brutal?: boolean | null;
  brutal_dl_speed?: number | null;
  brutal_up_speed?: number | null;
  // VMess
  alter_id?: number;
  security?: string;
  // Trojan
  password?: string;
  // Shadowsocks
  encryption?: string;
  plugin?: string | null;
  plugin_args?: string | null;
  udp_over_tcp?: boolean;
  // Hysteria2
  server_ports?: string | null;
  hop_interval?: string | null;
  up_mbps?: number;
  down_mbps?: number;
  obf_password?: string | null;
  // TUIC
  congestion_control?: string;
  udp_relay_mode?: string;
  udp_over_stream?: boolean;
  zero_rtt_handshake?: boolean;
  heartbeat?: string | null;
  // SOCKS
  version?: number;
  // HTTP
  username?: string;
  // Chain
  proxies?: string[];
  // Advanced Dial Fields
  advanced?: boolean | null;
  reuse_address?: boolean | null;
  tcp_fast_open?: boolean | null;
  udp_fragment?: boolean | null;
  tcp_multi_path?: boolean | null;
  connect_timeout?: number | null;
  // Advanced TLS Fields
  tls_disable_sni?: boolean | null;
  tls_min_version?: string | null;
  tls_max_version?: string | null;
  tls_enable_ech?: boolean | null;
  tls_ech_config?: string | null;
  tls_certificate_sha256?: string | null;
  tls_client_cert?: string | null;
  tls_client_key?: string | null;
};

export type CreateProxyInput = {
  title: string;
  config: ProxyServerConfig;
};

/// Result of a subscription sync. Returned on foreground refresh; persisted on
/// the profile (with `unread_summary`) on background refresh.
export type SyncSummary = {
  proxies_added: number;
  proxies_removed: number;
  proxies_kept: number;
  rules_orphaned: number;
  at_unix_ms: number;
  errors: string[];
};

/// Registry entry describing a profile (the routing bundle is loaded on demand).
export type ProfileMeta = {
  id: string;
  name: string;
  subscription_url?: string | null;
  last_updated_unix_ms?: number | null;
  is_active: boolean;
  last_sync_summary?: SyncSummary | null;
  unread_summary: boolean;
  skip_auto_update: boolean;
  update_interval_hours: number;
  // Subscription usage from `subscription-userinfo` (null until first sync).
  // Convention: total === 0 means unlimited, expire === 0 means never expires.
  sub_upload?: number | null;
  sub_download?: number | null;
  sub_total?: number | null;
  sub_expire?: number | null;
  // Name of a hugeicons icon (see $lib/core/profile-icons) chosen for this
  // profile. Null/undefined falls back to an initial-letter avatar.
  icon?: string | null;
};

export type CreateProfileInput = {
  // Optional: user name wins; else the profile-title header; else a fallback.
  name?: string | null;
  subscription_url?: string | null;
  skip_auto_update?: boolean;
  update_interval_hours?: number;
  icon?: string | null;
};

export type UpdateProfileInput = {
  id: string;
  name?: string | null;
  /// Some(value) sets the url, Some(null) clears it, undefined leaves it.
  subscription_url?: string | null;
  skip_auto_update?: boolean;
  update_interval_hours?: number;
  /// Some(value) sets the icon, Some(null) clears it, undefined leaves it.
  icon?: string | null;
  dns?: DnsConfig;
};

export type DnsServer = {
  tag: string;
  address: string;
  detour?: string | null;
  name: string;
};

export type DnsRuleKind = "domain" | "domain_suffix" | "ruleset";

/** A configured DNS server tag, or the backend's built-in block action. */
export type DnsRuleTarget = DnsServer["tag"] | "block";

export type DnsHostEntry = {
  id: string;
  domain: string;
  address: string;
  enabled: boolean;
};

export type DnsConfig = {
  servers?: DnsServer[];
  final_server?: string | null;
  fake_ip?: boolean;
  rules?: DnsRuleDto[];
  hosts?: DnsHostEntry[];
};

/** A DNS routing rule persisted on the profile. */
export type DnsRuleDto = {
  id: string;
  kind: DnsRuleKind;
  /** A domain, domain suffix, or enabled ruleset tag, depending on `kind`. */
  value: string;
  target: DnsRuleTarget;
  comment?: string | null;
  enabled: boolean;
};

export type RuleSet = {
  id: string;
  tag: string;
  type: string;
  source: string;
  action: string;
  comment?: string | null;
  /** Policy tag for remote ruleset download detour */
  download_policy?: string | null;
  /** When false, omitted from core config / match. */
  enabled: boolean;
  /** Human-readable label for this ruleset (shown in UI selectors). */
  name?: string | null;
};

export type Rule = {
  id: number;
  kind: RuleType;
  value: string;
  outbound: string;
  comment: string;
  rule_set?: string | null;
  /// True when this rule's target proxy was deleted and it was repointed to the
  /// default final outbound. Surfaced as a warning in the rule table.
  orphaned?: boolean;
  /** When false, not pushed to live overlay. Default true. */
  enabled?: boolean;
};

/** Permanent-shaped rule with absolute expiry (unix ms). */
export type TempRule = Rule & {
  expires_at: number;
  /** Session rule: no expiry (expires_at=0), dies on core reload. */
  session?: boolean;
  /** Core ask session-cache key — used to invalidate on delete/edit. */
  ask_group_key?: string;
};

export type CoreProfile = {
  id: string;
  version: number;
  outbound_mode: OutboundMode;
  final_policy?: string | null;
  local_proxy: LocalProxyConfig;
  dns?: DnsConfig;
  rule_sets?: RuleSet[];
  policies: ProxyPolicy[];
  proxies: Proxy[];
  rules: Rule[];
  temp_rules?: TempRule[];
  /** Volatile session rules (ask-remember / "Session" temp type), in-memory only. */
  session_rules?: TempRule[];
};

export type CorePathsInfo = {
  app_data_dir: string;
  resource_dir?: string | null;
  core_dir: string;
  bin_dir: string;
  platform_bin_dir: string;
  runtime_dir: string;
  config_dir: string;
  profile_path: string;
};

export type BinaryCheck = {
  name: string;
  path: string | null;
  found: boolean;
  executable: boolean;
  required?: boolean;
  version: string | null;
  error: string | null;
};

export type CorePreflightReport = {
  ok: boolean;
  platform: string;
  paths: CorePathsInfo;
  checks: BinaryCheck[];
  errors: string[];
};

export type CoreRuntimeStatus = {
  phase: RuntimePhase;
  message: string | null;
  zeytun_core_pid: number | null;
};

/// Live core lifecycle as reported by the daemon's gRPC link (source of truth).
/// Payload of the `core-status-changed` event.
export type CoreStatus = "connecting" | "running" | "stopped";

export type TrafficPoint = { ts: number; up: number; down: number };
export type TrafficTopEntry = { name: string; up: number; down: number };
export type TrafficAnalytics = {
  series: TrafficPoint[];
  top_domains: TrafficTopEntry[];
  top_policies: TrafficTopEntry[];
  top_processes: TrafficTopEntry[];
};
export type TrafficFilter = "all" | "proxy";
export type TrafficRange = "1h" | "6h" | "24h" | "7d" | "30d";
export type TrafficCategory = "domain" | "policy" | "process";

export type TrafficSummary = { direct_bytes: number; proxy_bytes: number };
export type TrafficPeriod = "today" | "month";

export type ConnStatus = "active" | "completed" | "error";
export type ConnDto = {
  id: string;
  status: ConnStatus;
  policy: string;
  isProxy: boolean;
  processName: string;
  processPath: string;
  host: string;
  address: string;
  network: string;
  protocol: string;
  rule: string;
  upBytes: number;
  downBytes: number;
  createdAt: number;
  endedAt: number | null;
  durationMs: number;
};
export type InspectorSnapshot = { active: ConnDto[]; recent: ConnDto[] };

// --- CRUD Command DTOs ---

export type UpdateProxyInput = {
  tag: string;
  title?: string | null;
  config?: ProxyServerConfig | null;
  enabled?: boolean | null;
};

export type CreatePolicyInput = {
  name: string;
  kind: ProxyPolicyType;
  members: string[];
  selected_member_tag?: string | null;
  test_url?: string | null;
  interval_seconds?: number | null;
  tolerance_ms?: number | null;
  strategy?: string | null;
  weights?: number[] | null;
};

export type UpdatePolicyInput = {
  tag: string;
  name?: string | null;
  kind?: ProxyPolicyType | null;
  members?: string[] | null;
  selected_member_tag?: string | null;
  test_url?: string | null;
  interval_seconds?: number | null;
  tolerance_ms?: number | null;
  strategy?: string | null;
  weights?: number[] | null;
};

export type SelectPolicyMemberInput = {
  policy_tag: string;
  member_tag: string;
};

export type ClashProxyInfo = {
  type: string;
  name: string;
  now?: string | null;
  all?: string[] | null;
  history: { time: string; delay: number }[];
};

export type ClashProxiesResponse = {
  proxies: Record<string, ClashProxyInfo>;
};

export type ClashDelayResult = {
  delay?: number | null;
};

export type ClashConnectionMetadata = {
  network: string;
  type: string;
  sourceIP: string;
  destinationIP: string;
  destinationPort: string;
  host: string;
  processPath: string;
  process: string;
  pid?: number;
};

export type ClashConnection = {
  id: string;
  metadata: ClashConnectionMetadata;
  upload: number;
  download: number;
  start: string;
  chains: string[];
  rule: string;
  rulePayload: string;
};

export type ClashConnectionsResponse = {
  downloadTotal: number;
  uploadTotal: number;
  connections: ClashConnection[];
};

export interface ProcessHistoryEntry {
  name: string;
  path: string;
  totalConnections: number;
  totalUpload: number;
  totalDownload: number;
  lastSeenAt: number;
}
