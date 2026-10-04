use serde::Serialize;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::Mutex;
use tokio::time::{interval, Duration};
use tokio_stream::StreamExt;

#[allow(clippy::all)]
pub mod pb {
    tonic::include_proto!("daemon");
}

use pb::started_service_client::StartedServiceClient;
use pb::{ConnectionEventType, SubscribeConnectionsRequest};

use crate::core::dto::{ConnRecord, ConnStatus};

/// Per-executable breakdown within an app bundle.
#[derive(Clone, Serialize, Default, Debug)]
pub struct SubProcessStat {
    /// Executable name (last path component), e.g. "Google Chrome Helper (Renderer)"
    pub name: String,
    /// Full executable path
    pub path: String,
    #[serde(rename = "connectionIds")]
    pub connection_ids: Vec<String>,
    #[serde(rename = "uploadSpeed")]
    pub upload_speed: i64,
    #[serde(rename = "downloadSpeed")]
    pub download_speed: i64,
    #[serde(rename = "activeConnections")]
    pub active_connections: u32,
    #[serde(rename = "closedConnections")]
    pub closed_connections: u32,
    #[serde(rename = "totalUpload")]
    pub total_upload: i64,
    #[serde(rename = "totalDownload")]
    pub total_download: i64,
}

#[derive(Clone, Serialize, Default, Debug)]
pub struct ProcessStat {
    pub name: String,
    pub path: String,
    #[serde(rename = "connectionIds")]
    pub connection_ids: Vec<String>,
    #[serde(rename = "uploadSpeed")]
    pub upload_speed: i64,
    #[serde(rename = "downloadSpeed")]
    pub download_speed: i64,
    #[serde(rename = "activeConnections")]
    pub active_connections: u32,
    #[serde(rename = "closedConnections")]
    pub closed_connections: u32,
    #[serde(rename = "totalUploadSinceLaunch")]
    pub total_upload: i64,
    #[serde(rename = "totalDownloadSinceLaunch")]
    pub total_download: i64,
    #[serde(rename = "todayUpload")]
    pub today_upload: i64,
    #[serde(rename = "todayDownload")]
    pub today_download: i64,
    #[serde(rename = "topHost")]
    pub top_host: String,
    /// Per-executable breakdown (populated for .app bundles with multiple binaries).
    #[serde(rename = "subProcesses")]
    pub sub_processes: Vec<SubProcessStat>,
}

struct ConnectionRow {
    id: String,
    process_path: String,
    process_name: String,
    /// Last path component of the executable (e.g. "Google Chrome Helper (Renderer)")
    executable_name: String,
    host: String,
    /// Outbound tag the connection was routed through (analytics `policy` dim).
    policy: String,
    /// Whether traffic left through a proxy (vs direct/block).
    is_proxy: bool,
    uplink_rate: i64,
    downlink_rate: i64,
    uplink_total: i64,
    downlink_total: i64,
    closed_at: Option<i64>,
}

/// Buffer key: one bucket per minute × dimension combo. Owned strings are cloned
/// once per new key (per minute), not per tick.
#[derive(Clone, PartialEq, Eq, Hash)]
struct AnalyticsKey {
    ts_minute: i64,
    domain: String,
    policy: String,
    process: String,
    is_proxy: bool,
}

/// In-memory accumulation of traffic deltas, flushed to SQLite every
/// `ANALYTICS_FLUSH_SECS`. Shared between the stream loop (writer) and the flush
/// task (drainer).
type AnalyticsBuffer = std::sync::Mutex<HashMap<AnalyticsKey, (i64, i64)>>;

const ANALYTICS_FLUSH_SECS: u64 = 30;

#[derive(Clone)]
pub struct AuthInterceptor;

impl tonic::service::Interceptor for AuthInterceptor {
    fn call(
        &mut self,
        mut request: tonic::Request<()>,
    ) -> Result<tonic::Request<()>, tonic::Status> {
        let token = tonic::metadata::MetadataValue::try_from("Bearer ZEYTUN-API-SECRET").unwrap();
        request.metadata_mut().insert("authorization", token);
        Ok(request)
    }
}

type AuthClient = StartedServiceClient<
    tonic::codegen::InterceptedService<tonic::transport::Channel, AuthInterceptor>,
>;

/// Drop cumulative stats for a process that has shown no activity for this long.
const CUMULATIVE_TTL_SECS: i64 = 30 * 60;
/// Hard backstop on the number of processes tracked, regardless of TTL.
const MAX_TRACKED_PROCESSES: usize = 1000;
/// Hard cap on the number of hosts tracked per process (we only surface the top one).
const MAX_TRACKED_HOSTS: usize = 256;

#[derive(Clone, Default)]
struct SubProcessCumulative {
    upload: i64,
    download: i64,
    closed_connections: u32,
    path: String,
}

#[derive(Clone, Default)]
struct ProcessCumulative {
    upload: i64,
    download: i64,
    today_upload: i64,
    today_download: i64,
    closed_connections: u32,
    host_traffic: HashMap<String, i64>,
    path: String,
    /// Epoch seconds of the last tick in which this process had activity. Used for TTL eviction.
    last_seen: i64,
    /// Per-executable cumulative stats within this app bundle, keyed by executable name.
    sub_processes: HashMap<String, SubProcessCumulative>,
}

/// True lifecycle of the zeytun-core core, derived from the daemon's gRPC link — the
/// daemon is the source of truth for whether traffic can actually flow.
/// Sent as a `&'static str` payload (no allocation).
const STATUS_CONNECTING: &str = "connecting";
const STATUS_RUNNING: &str = "running";
const STATUS_STOPPED: &str = "stopped";

/// Emit `core-status-changed` only when the status actually transitions, so the
/// frontend isn't spammed and the retry loop stays quiet while idle.
fn emit_status(app: &AppHandle, last: &mut &'static str, next: &'static str) {
    if *last == next {
        return;
    }
    *last = next;
    let _ = app.emit("core-status-changed", next);
}

pub fn start_daemon_listener(
    app: AppHandle,
) -> (
    tauri::async_runtime::JoinHandle<()>,
    tokio::sync::oneshot::Sender<()>,
) {
    let (tx, mut rx) = tokio::sync::oneshot::channel::<()>();
    let app_clone = app.clone();

    let handle = tauri::async_runtime::spawn(async move {
        let endpoint =
            tonic::transport::Channel::from_static(crate::core::constants::GRPC_API_ENDPOINT);
        let mut status: &'static str = STATUS_STOPPED;
        emit_status(&app_clone, &mut status, STATUS_CONNECTING);
        loop {
            tokio::select! {
                _ = &mut rx => {
                    println!("[Daemon] Shutdown signal received, exiting loop");
                    emit_status(&app_clone, &mut status, STATUS_STOPPED);
                    break;
                }
                connect_result = endpoint.connect() => {
                    match connect_result {
                        Ok(channel) => {
                            let client = StartedServiceClient::with_interceptor(channel, AuthInterceptor)
                                .max_decoding_message_size(256 * 1024 * 1024);
                            println!("[Daemon] Connected to zeytun-core gRPC (connections)");
                            emit_status(&app_clone, &mut status, STATUS_RUNNING);

                            tokio::select! {
                                _ = &mut rx => {
                                    println!("[Daemon] Shutdown signal received while connected, exiting");
                                    emit_status(&app_clone, &mut status, STATUS_STOPPED);
                                    break;
                                }
                                res = handle_connections(client.clone(), app_clone.clone()) => {
                                    if let Err(e) = res {
                                        println!("[Daemon] handle_connections error: {:?}", e);
                                    }
                                }
                                res2 = handle_logs(client.clone(), app_clone.clone()) => {
                                    if let Err(e) = res2 {
                                        println!("[Daemon] handle_logs error: {:?}", e);
                                    }
                                }
                                res3 = handle_core_events(client.clone(), app_clone.clone()) => {
                                    if let Err(e) = res3 {
                                        println!("[Daemon] handle_core_events error: {:?}", e);
                                    }
                                }
                            }

                            println!("[Daemon] Disconnected from zeytun-core gRPC, retrying...");
                            // Link dropped — show reconnecting; if the core is truly down the
                            // next connect attempt fails and flips us to stopped.
                            emit_status(&app_clone, &mut status, STATUS_CONNECTING);
                        }
                        Err(_) => {
                            emit_status(&app_clone, &mut status, STATUS_STOPPED);
                            // Silent retry
                            tokio::select! {
                                _ = &mut rx => {
                                    println!("[Daemon] Shutdown signal received during retry sleep, exiting");
                                    break;
                                }
                                _ = tokio::time::sleep(Duration::from_secs(crate::core::constants::DAEMON_RECONNECT_INTERVAL_SECS)) => {}
                            }
                        }
                    }
                }
            }
        }
    });

    (handle, tx)
}

async fn handle_logs(
    mut client: AuthClient,
    app: AppHandle,
) -> Result<(), Box<dyn std::error::Error>> {
    let request = tonic::Request::new(());
    let mut stream = client.subscribe_log(request).await?.into_inner();

    while let Some(message) = stream.next().await {
        match message {
            Ok(log_msg) => {
                use tauri::Emitter;
                #[derive(serde::Serialize, Clone)]
                struct LogEvent {
                    level: i32,
                    message: String,
                }
                for msg in log_msg.messages {
                    let _ = app.emit(
                        "core-log-event",
                        LogEvent {
                            level: msg.level,
                            message: msg.message,
                        },
                    );
                }
            }
            Err(e) => {
                println!("[Daemon] Logs stream error: {:?}", e);
                break;
            }
        }
    }
    Ok(())
}

async fn handle_core_events(
    mut client: AuthClient,
    app: AppHandle,
) -> Result<(), Box<dyn std::error::Error>> {
    let request = tonic::Request::new(pb::SubscribeEventRequest { scopes: vec![] });
    let mut stream = client.subscribe_event(request).await?.into_inner();

    while let Some(message) = stream.next().await {
        match message {
            Ok(ev) => {
                #[derive(serde::Serialize, Clone)]
                struct CoreEventPayload {
                    id: String,
                    ts_ms: i64,
                    scope: i32,
                    code: String,
                    severity: i32,
                    title: String,
                    message: String,
                    attrs: std::collections::HashMap<String, String>,
                }

                let mut resolved_title = ev.title.clone();
                if ev.scope == 3 {
                    // SCOPE_RULESET
                    if let Some(tag) = ev.attrs.get("tag") {
                        if let Some(state) = app.try_state::<crate::AppState>() {
                            let core = state.core_arc();
                            let core_lock = core.lock().await;
                            if let Some(rule_sets) = &core_lock.profile_get().rule_sets {
                                if let Some(rs) = rule_sets.iter().find(|r| r.tag == *tag) {
                                    let name_str = rs.name.as_deref().unwrap_or(tag.as_str());
                                    if ev.code.contains("FAILED") {
                                        resolved_title =
                                            format!("Ruleset '{}' update failed", name_str);
                                    } else {
                                        resolved_title = format!("Ruleset '{}' updated", name_str);
                                    }
                                }
                            }
                        }
                    }
                }

                // Debug ruleset download failures in app console / terminal.
                if ev.code == "RULESET_INITIAL_FETCH_FAILED" || ev.code == "RULESET_UPDATE_FAILED" {
                    let tag = ev.attrs.get("tag").map(String::as_str).unwrap_or("?");
                    let detour = ev.attrs.get("detour").map(String::as_str).unwrap_or("?");
                    let url = ev.attrs.get("url").map(String::as_str).unwrap_or("?");
                    let err = if !ev.message.is_empty() {
                        ev.message.as_str()
                    } else {
                        ev.attrs.get("error").map(String::as_str).unwrap_or("")
                    };
                    eprintln!(
                        "[ruleset] {code} tag={tag} detour={detour} url={url} err={err}",
                        code = ev.code,
                    );
                }
                let _ = app.emit(
                    "core-notification",
                    CoreEventPayload {
                        id: ev.id,
                        ts_ms: ev.ts_ms,
                        scope: ev.scope,
                        code: ev.code,
                        severity: ev.severity,
                        title: resolved_title,
                        message: ev.message,
                        attrs: ev.attrs,
                    },
                );
            }
            Err(e) => {
                println!("[Daemon] CoreEvent stream error: {:?}", e);
                break;
            }
        }
    }
    Ok(())
}

async fn handle_connections(
    mut client: AuthClient,
    app: AppHandle,
) -> Result<(), Box<dyn std::error::Error>> {
    let request = tonic::Request::new(SubscribeConnectionsRequest {
        interval: 1_000_000_000,
    }); // 1 second
    let mut stream = client.subscribe_connections(request).await?.into_inner();

    let rows = Arc::new(Mutex::new(HashMap::<String, ConnectionRow>::new()));
    let analytics_buf: Arc<AnalyticsBuffer> = Arc::new(std::sync::Mutex::new(HashMap::new()));

    // Spawn updater
    let rows_for_emit = rows.clone();
    let app_for_emit = app.clone();

    struct AbortOnDrop(tauri::async_runtime::JoinHandle<()>);
    impl Drop for AbortOnDrop {
        fn drop(&mut self) {
            self.0.abort();
        }
    }

    // Periodic flush of the analytics buffer to SQLite. Drains the buffer, then
    // persists via spawn_blocking + blocking_lock (no guard across await).
    let buf_for_flush = analytics_buf.clone();
    let app_for_flush = app.clone();
    let flush_task = tauri::async_runtime::spawn(async move {
        let mut timer = interval(Duration::from_secs(ANALYTICS_FLUSH_SECS));
        loop {
            timer.tick().await;
            let batch: Vec<crate::core::dto::TrafficDelta> = {
                let mut buf = match buf_for_flush.lock() {
                    Ok(b) => b,
                    Err(_) => continue,
                };
                if buf.is_empty() {
                    continue;
                }
                std::mem::take(&mut *buf)
                    .into_iter()
                    .map(|(k, (up, down))| crate::core::dto::TrafficDelta {
                        ts_minute: k.ts_minute,
                        domain: k.domain,
                        policy: k.policy,
                        process: k.process,
                        is_proxy: k.is_proxy,
                        up_bytes: up,
                        down_bytes: down,
                    })
                    .collect()
            };

            let Some(state) = app_for_flush.try_state::<crate::AppState>() else {
                continue;
            };
            let core = state.core_arc();
            let _ = tauri::async_runtime::spawn_blocking(move || {
                let guard = core.blocking_lock();
                if let Err(e) = guard.record_traffic_batch(&batch) {
                    eprintln!("[Daemon] traffic flush failed: {e}");
                }
            })
            .await;
        }
    });
    let _flush_task_guard = AbortOnDrop(flush_task);

    let emit_task = tauri::async_runtime::spawn(async move {
        let mut interval_timer = interval(Duration::from_secs(1));

        let mut process_cumulative: HashMap<String, ProcessCumulative> = HashMap::new();
        let mut last_day = get_current_day();

        loop {
            interval_timer.tick().await;

            let now = get_current_epoch();
            let current_day = get_current_day();
            if current_day != last_day {
                for cum in process_cumulative.values_mut() {
                    cum.today_upload = 0;
                    cum.today_download = 0;
                }
                last_day = current_day;
            }

            let mut current_rows = rows_for_emit.lock().await;

            // Build ProcessStat
            let mut process_map: HashMap<String, ProcessStat> = HashMap::new();
            // Temporary per-executable live stats, keyed by (process_name, executable_name)
            let mut sub_live: HashMap<(String, String), SubProcessStat> = HashMap::new();

            for row in current_rows.values() {
                // If it's closed, we add its totals to cumulative and then we will remove it.
                if row.closed_at.is_some() {
                    let cum = process_cumulative
                        .entry(row.process_name.clone())
                        .or_default();
                    cum.upload += row.uplink_total;
                    cum.download += row.downlink_total;
                    cum.today_upload += row.uplink_total;
                    cum.today_download += row.downlink_total;
                    cum.closed_connections += 1;
                    cum.last_seen = now;
                    // Track per-executable cumulative
                    let sub_cum = cum
                        .sub_processes
                        .entry(row.executable_name.clone())
                        .or_default();
                    sub_cum.upload += row.uplink_total;
                    sub_cum.download += row.downlink_total;
                    sub_cum.closed_connections += 1;
                    if sub_cum.path.is_empty() {
                        sub_cum.path = row.process_path.clone();
                    }
                    continue;
                }

                let stat = process_map
                    .entry(row.process_name.clone())
                    .or_insert_with(|| ProcessStat {
                        name: row.process_name.clone(),
                        path: row.process_path.clone(),
                        connection_ids: Vec::new(),
                        upload_speed: 0,
                        download_speed: 0,
                        active_connections: 0,
                        closed_connections: 0,
                        total_upload: 0,
                        total_download: 0,
                        today_upload: 0,
                        today_download: 0,
                        top_host: "None".to_string(),
                        sub_processes: Vec::new(),
                    });

                stat.upload_speed += row.uplink_rate;
                stat.download_speed += row.downlink_rate;
                stat.active_connections += 1;
                stat.connection_ids.push(row.id.clone());

                // Keep track of current active total to add to cumulative when displaying
                stat.total_upload += row.uplink_total;
                stat.total_download += row.downlink_total;
                stat.today_upload += row.uplink_total;
                stat.today_download += row.downlink_total;

                // Track per-executable live stats
                let sub_key = (row.process_name.clone(), row.executable_name.clone());
                let sub = sub_live.entry(sub_key).or_insert_with(|| SubProcessStat {
                    name: row.executable_name.clone(),
                    path: row.process_path.clone(),
                    ..Default::default()
                });
                sub.upload_speed += row.uplink_rate;
                sub.download_speed += row.downlink_rate;
                sub.active_connections += 1;
                sub.connection_ids.push(row.id.clone());
                sub.total_upload += row.uplink_total;
                sub.total_download += row.downlink_total;

                let cum = process_cumulative
                    .entry(row.process_name.clone())
                    .or_default();
                cum.last_seen = now;
                if cum.path.is_empty() {
                    cum.path = stat.path.clone();
                }
                record_host_traffic(
                    &mut cum.host_traffic,
                    &row.host,
                    row.uplink_rate + row.downlink_rate,
                );
            }

            // Remove closed
            current_rows.retain(|_, row| row.closed_at.is_none());

            // Evict stale cumulative entries so the map can't grow without bound.
            evict_stale_processes(&mut process_cumulative, now);

            // Add cumulative to stats
            for (pname, cum) in process_cumulative.iter_mut() {
                let stat = process_map
                    .entry(pname.clone())
                    .or_insert_with(|| ProcessStat {
                        name: pname.clone(),
                        path: cum.path.clone(),
                        connection_ids: Vec::new(),
                        upload_speed: 0,
                        download_speed: 0,
                        active_connections: 0,
                        closed_connections: 0,
                        total_upload: 0,
                        total_download: 0,
                        today_upload: 0,
                        today_download: 0,
                        top_host: "None".to_string(),
                        sub_processes: Vec::new(),
                    });

                stat.total_upload += cum.upload;
                stat.total_download += cum.download;
                stat.today_upload += cum.today_upload;
                stat.today_download += cum.today_download;
                stat.closed_connections += cum.closed_connections;

                // calculate top host
                let mut max_t = -1;
                let mut top_h = "None".to_string();
                for (h, t) in cum.host_traffic.iter() {
                    if *t > max_t {
                        max_t = *t;
                        top_h = h.clone();
                    }
                }
                stat.top_host = top_h;

                // Merge per-executable cumulative into live sub-process stats
                for (exec_name, sub_cum) in cum.sub_processes.iter() {
                    let sub_key = (pname.clone(), exec_name.clone());
                    let sub = sub_live.entry(sub_key).or_insert_with(|| SubProcessStat {
                        name: exec_name.clone(),
                        path: sub_cum.path.clone(),
                        ..Default::default()
                    });
                    sub.total_upload += sub_cum.upload;
                    sub.total_download += sub_cum.download;
                    sub.closed_connections += sub_cum.closed_connections;
                }
            }

            // Attach sub-process stats to their parent ProcessStat
            for ((pname, _), sub) in sub_live.drain() {
                if let Some(stat) = process_map.get_mut(&pname) {
                    stat.sub_processes.push(sub);
                }
            }
            // Sort sub-processes by total traffic desc for consistent display
            for stat in process_map.values_mut() {
                stat.sub_processes.sort_by(|a, b| {
                    (b.total_upload + b.total_download).cmp(&(a.total_upload + a.total_download))
                });
                // Only keep sub_processes populated when there are 2+ distinct executables
                // (single-executable apps don't need the breakdown)
                if stat.sub_processes.len() <= 1 {
                    stat.sub_processes.clear();
                }
            }

            let stats_vec: Vec<ProcessStat> = process_map.into_values().collect();

            // Aggregate live up/down rate for the tray title (single pass, no clones).
            let (mut up, mut down) = (0i64, 0i64);
            for stat in &stats_vec {
                up += stat.upload_speed;
                down += stat.download_speed;
            }
            crate::tray::set_traffic_title(&app_for_emit, up, down);

            let _ = app_for_emit.emit("traffic-update", &stats_vec);

            // Push the Inspector snapshot to the frontend, but only while the
            // Inspector route is mounted (listener_active) AND the connection
            // set changed since the last tick — idle pages cost nothing.
            if let Some(ins) = app_for_emit.try_state::<crate::InspectorState>() {
                if ins
                    .listener_active
                    .load(std::sync::atomic::Ordering::Relaxed)
                    && ins.dirty.swap(false, std::sync::atomic::Ordering::Relaxed)
                {
                    match ins.snapshot() {
                        Ok(snap) => {
                            let _ = app_for_emit.emit("inspector-snapshot", &snap);
                        }
                        Err(e) => eprintln!("[Daemon] inspector snapshot failed: {e}"),
                    }
                }
            }
        }
    });

    let _emit_task_guard = AbortOnDrop(emit_task);

    // Per-connection Inspector tracking (independent of the ProcessStat aggregation).
    let inspector = app.try_state::<crate::InspectorState>();

    while let Some(message) = stream.next().await {
        let events_msg = match message {
            Ok(m) => m,
            Err(e) => {
                println!("[Daemon] Connections stream error: {:?}", e);
                break;
            }
        };

        let mut current_rows = rows.lock().await;

        if events_msg.reset {
            current_rows.clear();
            // A reset means the previous connection set is stale; retire any still
            // "active" Inspector rows into the recent buffer so `active` can't leak.
            if let Some(ins) = &inspector {
                if let (Ok(mut active), Ok(mut recent)) = (ins.active.lock(), ins.recent.lock()) {
                    let now = get_current_epoch();
                    for (_, mut rec) in active.drain() {
                        rec.ended_at = Some(now);
                        if rec.status != ConnStatus::Error {
                            rec.status = ConnStatus::Completed;
                        }
                        push_recent(&mut recent, rec);
                    }
                }
                ins.dirty.store(true, std::sync::atomic::Ordering::Relaxed);
            }
        }

        for event in events_msg.events {
            match event.r#type() {
                ConnectionEventType::ConnectionEventNew => {
                    if let Some(conn) = event.connection {
                        let process_path = conn
                            .process_info
                            .as_ref()
                            .map(|p| p.process_path.clone())
                            .unwrap_or_default();
                        let process_name = extract_process_name(&process_path);
                        let executable_name = extract_executable_name(&process_path);
                        let host = if !conn.domain.is_empty() {
                            conn.domain.clone()
                        } else {
                            conn.destination.clone()
                        };
                        let is_proxy =
                            is_proxied(&conn.outbound, &conn.outbound_type, &conn.chain_list);
                        let blocked = conn.outbound == "block";
                        let policy = if conn.outbound.is_empty() {
                            "direct".to_string()
                        } else {
                            conn.outbound.clone()
                        };

                        // Inspector record (clones owned strings once; remaining
                        // fields move into the ConnectionRow below).
                        if let Some(ins) = &inspector {
                            if let Ok(mut active) = ins.active.lock() {
                                active.insert(
                                    event.id.clone(),
                                    ConnRecord {
                                        id: event.id.clone(),
                                        status: if blocked {
                                            ConnStatus::Error
                                        } else {
                                            ConnStatus::Active
                                        },
                                        policy: policy.clone(),
                                        is_proxy,
                                        process_name: process_name.clone(),
                                        process_path: process_path.clone(),
                                        host: host.clone(),
                                        address: conn.destination.clone(),
                                        network: conn.network.clone(),
                                        protocol: conn.protocol.clone(),
                                        rule: conn.rule.clone(),
                                        up_bytes: conn.uplink_total,
                                        down_bytes: conn.downlink_total,
                                        // zeytun-core `created_at` is in non-second units
                                        // (yielded 0ms durations); stamp our own
                                        // epoch-seconds so it matches `ended_at`.
                                        created_at: get_current_epoch(),
                                        ended_at: None,
                                    },
                                );
                            }
                            ins.dirty.store(true, std::sync::atomic::Ordering::Relaxed);
                        }

                        current_rows.insert(
                            event.id.clone(),
                            ConnectionRow {
                                id: event.id,
                                process_path,
                                process_name,
                                executable_name,
                                host,
                                policy,
                                is_proxy,
                                uplink_rate: 0,
                                downlink_rate: 0,
                                uplink_total: conn.uplink_total,
                                downlink_total: conn.downlink_total,
                                closed_at: None,
                            },
                        );
                    }
                }
                ConnectionEventType::ConnectionEventUpdate => {
                    if let Some(row) = current_rows.get_mut(&event.id) {
                        row.uplink_rate = event.uplink_delta;
                        row.downlink_rate = event.downlink_delta;
                        row.uplink_total += event.uplink_delta;
                        row.downlink_total += event.downlink_delta;

                        // Accumulate the delta into the per-minute analytics bucket.
                        if event.uplink_delta != 0 || event.downlink_delta != 0 {
                            let key = AnalyticsKey {
                                ts_minute: get_current_epoch() / 60 * 60,
                                domain: row.host.clone(),
                                policy: row.policy.clone(),
                                process: row.process_name.clone(),
                                is_proxy: row.is_proxy,
                            };
                            if let Ok(mut buf) = analytics_buf.lock() {
                                let entry = buf.entry(key).or_insert((0, 0));
                                entry.0 += event.uplink_delta;
                                entry.1 += event.downlink_delta;
                            }
                        }
                    }

                    // Mirror the byte deltas into the Inspector record.
                    if let Some(ins) = &inspector {
                        if let Ok(mut active) = ins.active.lock() {
                            if let Some(rec) = active.get_mut(&event.id) {
                                rec.up_bytes += event.uplink_delta;
                                rec.down_bytes += event.downlink_delta;
                            }
                            ins.dirty.store(true, std::sync::atomic::Ordering::Relaxed);
                        }
                    }
                }
                ConnectionEventType::ConnectionEventClosed => {
                    if let Some(row) = current_rows.get_mut(&event.id) {
                        row.closed_at = Some(get_current_epoch());
                    }

                    // Move the Inspector record from active → recent (ring buffer).
                    if let Some(ins) = &inspector {
                        if let Ok(mut active) = ins.active.lock() {
                            if let Some(mut rec) = active.remove(&event.id) {
                                rec.ended_at = Some(get_current_epoch());
                                if rec.status != ConnStatus::Error {
                                    rec.status = ConnStatus::Completed;
                                }
                                drop(active);
                                if let Ok(mut recent) = ins.recent.lock() {
                                    push_recent(&mut recent, rec);
                                }
                            }
                            ins.dirty.store(true, std::sync::atomic::Ordering::Relaxed);
                        }
                    }
                }
            }
        }
    }

    Ok(())
}

/// Push a closed connection into the recent ring buffer, evicting the oldest
/// once the cap is exceeded. O(1) amortized; no reallocation churn.
fn push_recent(recent: &mut std::collections::VecDeque<ConnRecord>, rec: ConnRecord) {
    recent.push_back(rec);
    while recent.len() > crate::INSPECTOR_RECENT_CAP {
        recent.pop_front();
    }
}

/// Add `amount` to a host's running total, capping the number of distinct hosts tracked.
/// When the cap is reached, the lowest-traffic host is evicted to make room. Existing
/// hosts are updated in place, avoiding an allocation of the host string on the hot path.
fn record_host_traffic(host_traffic: &mut HashMap<String, i64>, host: &str, amount: i64) {
    if let Some(total) = host_traffic.get_mut(host) {
        *total += amount;
        return;
    }
    if host_traffic.len() >= MAX_TRACKED_HOSTS {
        if let Some(min_key) = host_traffic
            .iter()
            .min_by_key(|(_, total)| **total)
            .map(|(key, _)| key.clone())
        {
            host_traffic.remove(&min_key);
        }
    }
    host_traffic.insert(host.to_string(), amount);
}

/// Drop cumulative entries that haven't been active within the TTL, then enforce a hard
/// cap by evicting the least-recently-active entries. Keeps memory bounded over long sessions.
fn evict_stale_processes(map: &mut HashMap<String, ProcessCumulative>, now: i64) {
    map.retain(|_, cum| now - cum.last_seen <= CUMULATIVE_TTL_SECS);

    if map.len() > MAX_TRACKED_PROCESSES {
        let mut by_age: Vec<(String, i64)> =
            map.iter().map(|(k, v)| (k.clone(), v.last_seen)).collect();
        by_age.sort_by_key(|(_, last_seen)| *last_seen);
        let remove_count = map.len() - MAX_TRACKED_PROCESSES;
        for (key, _) in by_age.into_iter().take(remove_count) {
            map.remove(&key);
        }
    }
}

fn get_current_epoch() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
}

fn get_current_day() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
        / 86400
}

/// A connection counts as proxied unless it went out a direct/block outbound.
fn is_proxied(outbound: &str, outbound_type: &str, chain_list: &[String]) -> bool {
    let is_bypass = |s: &str| s == "direct" || s == "block";

    if !chain_list.is_empty() {
        return chain_list.iter().any(|node| !is_bypass(node));
    }

    !(outbound_type == "direct" || outbound.is_empty() || is_bypass(outbound))
}

fn extract_process_name(path: &str) -> String {
    if path.is_empty() {
        return "System".to_string();
    }

    if let Some(idx) = path.find(".app/") {
        let slice = &path[..idx];
        if let Some(last_slash) = slice.rfind('/') {
            return slice[last_slash + 1..].to_string();
        }
        return slice.to_string();
    }

    if let Some(last_slash) = path.rfind('/') {
        return path[last_slash + 1..].to_string();
    }

    path.to_string()
}

/// Extract the executable filename (last path component).
/// For `/Applications/Google Chrome.app/.../Google Chrome Helper (Renderer)`
/// returns `"Google Chrome Helper (Renderer)"`.
fn extract_executable_name(path: &str) -> String {
    if path.is_empty() {
        return "System".to_string();
    }
    if let Some(last_slash) = path.rfind('/') {
        return path[last_slash + 1..].to_string();
    }
    path.to_string()
}
