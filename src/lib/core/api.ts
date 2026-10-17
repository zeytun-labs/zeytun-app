import { invoke } from "@tauri-apps/api/core";
import type {
  ClashProxiesResponse,
  ClashDelayResult,
  CoreEvent,
  CorePreflightReport,
  CoreProfile,
  CoreRuntimeStatus,
  CreatePolicyInput,
  CreateProfileInput,
  CreateProxyInput,
  OutboundMode,
  ProfileMeta,
  Proxy,
  ProxyPolicy,
  Rule,
  ResolvedProcess,
  SelectPolicyMemberInput,
  SyncSummary,
  UpdateProfileInput,
  UpdateProxyInput,
  UpdatePolicyInput,
  ClashConnectionsResponse,
  DeleteSubscriptionInput,
  RefreshResult,
  SubscriptionDeletePreview,
  SubscriptionSettings,
  SubscriptionSource,
  ExportMode,
  ImportPreview,
} from "./types";

export function corePreflight() {
  return invoke<CorePreflightReport>("core_preflight");
}

export function coreProfileGet() {
  return invoke<CoreProfile>("core_profile_get");
}

export function coreReset() {
  return invoke<CoreProfile>("core_reset");
}

export function coreProxyImportLink(link: string) {
  return invoke<Proxy>("core_proxy_import_link", { link });
}

export function coreProxyCreate(input: CreateProxyInput) {
  return invoke<Proxy>("core_proxy_create", { input });
}

export function coreStart() {
  return invoke<CoreRuntimeStatus>("core_start");
}

export function coreStop() {
  return invoke<CoreRuntimeStatus>("core_stop");
}

export function coreRestart() {
  return invoke<CoreRuntimeStatus>("core_restart");
}

export function coreStatus() {
  return invoke<CoreRuntimeStatus>("core_status");
}

export function coreEventList() {
  return invoke<CoreEvent[]>("core_event_list");
}

export function coreGetTrafficAnalytics(
  filter: "all" | "proxy",
  range: import("./types").TrafficRange = "24h",
) {
  return invoke<import("./types").TrafficAnalytics>(
    "core_get_traffic_analytics",
    { filter, range },
  );
}

export function coreGetTrafficSummary(period: "today" | "month") {
  return invoke<import("./types").TrafficSummary>("core_get_traffic_summary", {
    period,
  });
}

/** Toggle the daemon's inspector push feed (set by the Inspector route on
 *  mount/destroy — events only flow while the page is open). */
export function coreInspectorSetActive(active: boolean) {
  return invoke<void>("core_inspector_set_active", { active });
}

export function coreInspectorSnapshot() {
  return invoke<import("./types").InspectorSnapshot>("core_inspector_snapshot");
}

export function coreInspectorClear() {
  return invoke<void>("core_inspector_clear");
}

// --- Profiles ---

export function coreProfileList() {
  return invoke<ProfileMeta[]>("core_profile_list");
}

export function coreProfileCreate(input: CreateProfileInput) {
  return invoke<ProfileMeta>("core_profile_create", { input });
}

export function coreProfileUpdate(input: UpdateProfileInput) {
  return invoke<ProfileMeta>("core_profile_update", { input });
}

export function coreProfileDelete(profileId: string) {
  return invoke<void>("core_profile_delete", { profile_id: profileId });
}

export function coreProfileSwitch(profileId: string) {
  return invoke<void>("core_profile_switch", { profile_id: profileId });
}

export function coreProfileRefresh(profileId: string, background = false) {
  return invoke<SyncSummary>("core_profile_refresh", {
    profile_id: profileId,
    background,
  });
}

export function coreProfileMarkSummaryRead(profileId: string) {
  return invoke<void>("core_profile_mark_summary_read", {
    profile_id: profileId,
  });
}

export function coreProfileExport(profileId: string, mode: ExportMode) {
  return invoke<string>("core_profile_export", {
    profile_id: profileId,
    mode,
  });
}

export function coreProfileImportPreview(rawDocument: string) {
  return invoke<ImportPreview>("core_profile_import_preview", {
    raw_document: rawDocument,
  });
}

export function coreProfileImportConfirm(
  rawDocument: string,
  previewToken: string,
  customName?: string | null,
) {
  return invoke<ProfileMeta>("core_profile_import_confirm", {
    raw_document: rawDocument,
    preview_token: previewToken,
    custom_name: customName ?? null,
  });
}

// --- Subscription management commands ---

export function coreSubscriptionList(profileId: string) {
  return invoke<SubscriptionSource[]>("core_subscription_list", {
    profile_id: profileId,
  });
}

export function coreSubscriptionCreate(
  profileId: string,
  settings: SubscriptionSettings,
) {
  return invoke<SubscriptionSource>("core_subscription_create", {
    profile_id: profileId,
    settings,
  });
}

export function coreSubscriptionUpdate(
  profileId: string,
  sourceId: string,
  settings: SubscriptionSettings,
) {
  return invoke<SubscriptionSource>("core_subscription_update", {
    profile_id: profileId,
    source_id: sourceId,
    settings,
  });
}

export function coreSubscriptionRefresh(profileId: string, sourceId: string) {
  return invoke<RefreshResult>("core_subscription_refresh", {
    profile_id: profileId,
    source_id: sourceId,
  });
}

export function coreSubscriptionRefreshAll(profileId: string) {
  return invoke<RefreshResult[]>("core_subscription_refresh_all", {
    profile_id: profileId,
  });
}

export function coreSubscriptionDeletePreview(
  profileId: string,
  sourceId: string,
) {
  return invoke<SubscriptionDeletePreview>("core_subscription_delete_preview", {
    profile_id: profileId,
    source_id: sourceId,
  });
}

export function coreSubscriptionDelete(input: DeleteSubscriptionInput) {
  return invoke<void>("core_subscription_delete", { input });
}

export function coreSubscriptionMarkSummaryRead(
  profileId: string,
  sourceId: string,
) {
  return invoke<void>("core_subscription_mark_summary_read", {
    profile_id: profileId,
    source_id: sourceId,
  });
}

export function coreSubscriptionJobStatus(jobId: string) {
  return invoke<boolean>("core_subscription_job_status", { job_id: jobId });
}

export function coreSubscriptionJobCancel(jobId: string) {
  return invoke<void>("core_subscription_job_cancel", { job_id: jobId });
}

// --- Dedicated CRUD commands ---

export function coreSetMode(mode: OutboundMode) {
  return invoke<void>("core_set_mode", { mode });
}

export function coreSetInboundMode(mode: "mixed" | "tun") {
  return invoke<void>("core_set_inbound_mode", { mode });
}

export function coreSetSystemProxy(enabled: boolean) {
  return invoke<void>("core_set_system_proxy", { enabled });
}

export function coreSetListenPort(port: number): Promise<void> {
  return invoke("core_set_listen_port", { port });
}

export function coreSetLogLevel(level: string): Promise<void> {
  return invoke("core_set_log_level", { level });
}

export function coreSetAllowLan(enabled: boolean): Promise<void> {
  return invoke<void>("core_set_allow_lan", { enabled });
}

export function appIsElevated(): Promise<boolean> {
  return invoke<boolean>("app_is_elevated");
}

export function appRequestElevation(): Promise<void> {
  return invoke<void>("app_request_elevation");
}

export function coreUpdateProxy(input: UpdateProxyInput) {
  return invoke<Proxy>("core_update_proxy", { input });
}

export function coreDeleteProxy(proxyId: string) {
  return invoke<void>("core_delete_proxy", { proxy_id: proxyId });
}

export function coreProxyExportLink(proxyId: string) {
  return invoke<string>("core_proxy_export_link", { proxy_id: proxyId });
}

export function coreCreatePolicy(input: CreatePolicyInput) {
  return invoke<ProxyPolicy>("core_create_policy", { input });
}

export function coreUpdatePolicy(input: UpdatePolicyInput) {
  return invoke<ProxyPolicy>("core_update_policy", { input });
}

export function coreDeletePolicy(policyId: string) {
  return invoke<void>("core_delete_policy", { policy_id: policyId });
}

export function coreSelectPolicyMember(policy_tag: string, member_tag: string) {
  return invoke<void>("core_select_policy_member", {
    input: { policy_tag, member_tag } satisfies SelectPolicyMemberInput,
  });
}

export function coreUpdateRules(rules: Rule[]) {
  return invoke<void>("core_update_rules", { rules });
}

export function coreUpdateTempRules(tempRules: import("./types").TempRule[]) {
  return invoke<void>("core_update_temp_rules", { temp_rules: tempRules });
}

export function corePruneExpiredTempRules() {
  return invoke<boolean>("core_prune_expired_temp_rules");
}

export function corePromoteTempRule(id: number) {
  return invoke<void>("core_promote_temp_rule", { id });
}

export function coreSetConnectionAsk(config: {
  enabled: boolean;
  timeout_ms: number;
  group_by: string;
  remember_default: boolean;
}) {
  return invoke<void>("core_set_connection_ask", { config });
}

/** Traffic classes whose route the user can pick. Must match `TRAFFIC_KINDS` in Rust. */
export type TrafficKind = "app_update" | "geoip" | "subscription";

/** `direct` = no proxy; `local_proxy` = via the mixed inbound; `policy` = named policy. */
export type NetworkPolicy =
  | { kind: "direct" }
  | { kind: "local_proxy" }
  | { kind: "policy"; tag: string };

/**
 * Stored route per traffic class. Classes the user never touched are ABSENT from
 * the map — treat a missing key as `{ kind: "direct" }`.
 */
export function coreNetworkPolicyGet() {
  return invoke<Partial<Record<TrafficKind, NetworkPolicy>>>("core_network_policy_get");
}

export function coreNetworkPolicySet(traffic: TrafficKind, policy: NetworkPolicy) {
  return invoke<void>("core_network_policy_set", { traffic, policy });
}

/** Result of a GeoIP database download. */
export interface GeoipUpdate {
  /** `YYYY-MM` of the published file that was installed. */
  month: string;
  /** True when the configured route failed and the direct one was used. */
  degraded_to_direct: boolean;
}

export function coreUpdateGeoipDb() {
  return invoke<GeoipUpdate>("core_update_geoip_db");
}

/**
 * Outcome of a background GeoIP refresh. Only `updated` warrants a toast; the
 * rest are silent no-ops (already current, or the CDN was unreachable).
 */
export type GeoipAutoUpdate =
  | { kind: "updated"; month: string; degraded_to_direct: boolean }
  | { kind: "current"; month: string | null }
  | { kind: "probe_failed" }
  | { kind: "update_failed"; reason: string };

export function coreAutoUpdateGeoipDb() {
  return invoke<GeoipAutoUpdate>("core_auto_update_geoip_db");
}

export function coreDecideConnectionAsk(input: {
  id: string;
  outbound: string;
  reject: boolean;
  remember: boolean;
  process_path?: string | null;
  process_bundle?: string | null;
  dest_host?: string | null;
  group_by?: string | null;
}) {
  return invoke<void>("core_decide_connection_ask", input);
}

export function coreOpenConnectionAskWindow() {
  return invoke<void>("core_open_connection_ask_window");
}

export function coreUpdateRulesets(ruleSets: import("./types").RuleSet[]) {
  return invoke<void>("core_update_rulesets", { rule_sets: ruleSets });
}

export function coreUpdateDnsRules(dnsRules: import("./types").DnsRuleDto[]) {
  return invoke<void>("core_update_dns_rules", { dns_rules: dnsRules });
}

export function coreUpdateDnsHosts(hosts: import("./types").DnsHostEntry[]) {
  return invoke<void>("core_update_dns_hosts", { hosts });
}

/**
 * Set `dns.final` — the resolver for queries no DNS rule matched.
 * `null` means the built-in local (system) resolver.
 */
export function coreUpdateDnsFinal(finalServer: string | null) {
  return invoke<void>("core_update_dns_final", { final_server: finalServer });
}

/** Write .srs bytes to app data; returns absolute path for draft local rulesets. */
export function coreStageRulesetFile(fileBytes: number[]) {
  return invoke<string>("core_stage_ruleset_file", { file_bytes: fileBytes });
}

export function coreCreateRuleset(input: {
  kind: string;
  source: string;
  action: string;
  comment: string | null;
  file_bytes: number[] | null;
}) {
  return invoke<import("./types").RuleSet>("core_create_ruleset", input);
}

export function coreDeleteRuleset(id: string) {
  return invoke<void>("core_delete_ruleset", { id });
}

export function coreReadActiveConfig() {
  return invoke<string>("core_read_active_config");
}

export function coreRetryRulesets(policyTag: string) {
  return invoke<void>("core_retry_rulesets", { policy_tag: policyTag });
}

// --- Clash API commands ---

export function coreClashProxies() {
  return invoke<ClashProxiesResponse>("core_clash_proxies");
}

export function coreClashSelectProxy(policyTag: string, proxy: string) {
  return invoke<void>("core_clash_select_proxy", { group: policyTag, proxy });
}

export function coreClashSetMode(mode: string) {
  return invoke<void>("core_clash_set_mode", { mode });
}

export function coreClashProxyDelayTest(proxy: string, url: string, timeoutMs = 5000) {
  return invoke<ClashDelayResult>("core_clash_proxy_delay_test", {
    proxy,
    url,
    timeoutMs,
  });
}

export function coreRules() {
  return invoke<{ rules: unknown[] }>("core_rules");
}


export function coreClashApiConfig() {
  return invoke<{ controller: string; secret: string | null }>("core_clash_api_config");
}

export function getProcessIcon(processName: string, processPath: string) {
  return invoke<string>("get_process_icon", { processName, processPath });
}

export function coreResolveProcess(kind: string, value: string) {
  return invoke<ResolvedProcess | null>("core_resolve_process", { kind, value });
}



export function coreCloseConnections(ids: string[]) {
  return invoke<void>("core_close_connections", { ids });
}

export interface IpInfo {
  ip: string;
  emoji: string;
}

export function coreGetExternalIpInfo() {
  return invoke<IpInfo>("core_get_external_ip_info");
}




export function coreGetActiveNetworkInterface() {
  return invoke<string>("core_get_active_network_interface");
}

export interface LocalNetworkInfo {
  id: string;
  name: string;
  networkType: string;
  localIp: string;
}

export function coreGetLocalNetworkInfo() {
  return invoke<LocalNetworkInfo>("core_get_local_network_info");
}

export function coreGetLocalLatencies() {
  return invoke<{ router: number; dns: number }>("core_get_local_latencies");
}

export function coreRunDiagnostics(internetTestUrl: string, proxyTestUrl: string) {
  return invoke<import("./types").DiagnosticsResult>("core_run_diagnostics", { internetTestUrl, proxyTestUrl });
}

export function flushDnsCache() {
  return invoke<string>("flush_dns_cache");
}

export type DnsRecordType = "A" | "AAAA" | "CNAME";

export interface DnsRecord {
  name: string;
  record_type: string;
  data: string;
  ttl: number | null;
  section: "answer" | "authority" | "additional";
}

export interface DnsLookupTrace {
  server: string | null;
  transport: string | null;
  rule: string | null;
  rule_index: number | null;
  outbound: string | null;
  cache_hit: boolean | null;
  source: string | null;
  selection: string | null;
}

export interface DnsLookupResult {
  resolved_ips: string[];
  server_used: string;
  latency_ms: number;
  status: number | null;
  record_type: string;
  ttl: number | null;
  records: DnsRecord[];
  trace: DnsLookupTrace | null;
}

export function testDnsLookup(
  domain: string,
  recordType: DnsRecordType = "A",
  server: string | null = null,
  bypassCache = false,
) {
  return invoke<DnsLookupResult>("test_dns_lookup", { domain, recordType, server, bypassCache });
}

export function testSystemDnsLookup(domain: string, recordType: DnsRecordType = "A") {
  return invoke<DnsLookupResult>("test_system_dns_lookup", { domain, recordType });
}

export interface NetworkQualityTestProgressEvent {
  phase: number;
  downloadCapacity: number;
  uploadCapacity: number;
  downloadRpm: number;
  uploadRpm: number;
  idleLatencyMs: number;
  elapsedMs: number;
  isFinal: boolean;
  error: string;
  downloadCapacityAccuracy: number;
  uploadCapacityAccuracy: number;
  downloadRpmAccuracy: number;
  uploadRpmAccuracy: number;
}

export function coreNetworkQualityTestStart(
  configUrl: string,
  outboundTag: string,
  serial: boolean,
  maxRuntimeSeconds: number,
  http3: boolean,
) {
  return invoke<void>("core_network_quality_test_start", {
    configUrl,
    outboundTag,
    serial,
    maxRuntimeSeconds,
    http3,
  });
}

export function coreNetworkQualityTestCancel() {
  return invoke<void>("core_network_quality_test_cancel");
}

export interface StunTestProgressEvent {
  phase: number;
  externalAddr: string;
  latencyMs: number;
  natMapping: number;
  natFiltering: number;
  isFinal: boolean;
  error: string;
  natTypeSupported: boolean;
}

export function coreStunTestStart(server: string, outboundTag: string): Promise<void> {
  return invoke<void>("core_stun_test_start", { server, outboundTag });
}

export function coreStunTestCancel() {
  return invoke<void>("core_stun_test_cancel");
}

export function coreSetHideOnClose(hide: boolean) {
  return invoke<void>("set_hide_on_close", { hide });
}

export function crashDumpListUnseen(): Promise<string[]> {
  return invoke<string[]>("crash_dump_list_unseen");
}

export function crashDumpRead(path: string): Promise<string> {
  return invoke<string>("crash_dump_read", { path });
}

export function crashDumpDismiss(): Promise<void> {
  return invoke<void>("crash_dump_dismiss");
}
