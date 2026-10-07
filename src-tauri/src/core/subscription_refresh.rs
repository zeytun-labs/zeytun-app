//! Source-scoped refreshes. Only snapshot/commit hold CoreManager; HTTP and parsing never do.
use super::{
    dto::{SubscriptionSource, SyncSummary},
    manager::CoreManager,
    subscription::SubscriptionManager,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc, Mutex,
    },
};
use tokio::sync::{Notify, Semaphore};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RefreshStatus {
    Accepted,
    Failed,
    Stale,
    Cancelled,
    Paused,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefreshResult {
    pub profile_id: String,
    pub source_id: String,
    pub status: RefreshStatus,
    pub summary: Option<SyncSummary>,
    pub config_revision: u64,
}
struct Flight {
    generation: u64,
    sequence: u64,
    done: Notify,
    result: Mutex<Option<RefreshResult>>,
}
#[derive(Clone)]
struct Retry {
    generation: u64,
    failures: u32,
    next_at: i64,
}
type Key = (String, String);
pub struct RefreshWorkers {
    core: Arc<tokio::sync::Mutex<CoreManager>>,
    pool: Arc<Semaphore>,
    flights: Mutex<HashMap<Key, Arc<Flight>>>,
    retries: Mutex<HashMap<Key, Retry>>,
    sequence: AtomicU64,
    stopped: AtomicBool,
    idle: Notify,
    active: AtomicU64,
    jobs: Mutex<HashMap<String, Arc<AtomicBool>>>,
}
impl RefreshWorkers {
    pub fn new(core: Arc<tokio::sync::Mutex<CoreManager>>) -> Self {
        Self {
            core,
            pool: Arc::new(Semaphore::new(4)),
            flights: Mutex::new(HashMap::new()),
            retries: Mutex::new(HashMap::new()),
            sequence: AtomicU64::new(0),
            stopped: AtomicBool::new(false),
            idle: Notify::new(),
            active: AtomicU64::new(0),
            jobs: Mutex::new(HashMap::new()),
        }
    }

    pub fn cancel_job(&self, job_id: &str) -> bool {
        if let Some(cancel) = self.jobs.lock().unwrap().get(job_id) {
            cancel.store(true, Ordering::SeqCst);
            true
        } else {
            false
        }
    }

    pub fn is_job_cancelled(&self, job_id: &str) -> bool {
        self.jobs
            .lock()
            .unwrap()
            .get(job_id)
            .is_some_and(|c| c.load(Ordering::SeqCst))
    }

    pub fn register_job(&self, job_id: String, cancel: Arc<AtomicBool>) {
        self.jobs.lock().unwrap().insert(job_id, cancel);
    }
    pub async fn refresh(
        self: &Arc<Self>,
        profile_id: String,
        source_id: String,
        background: bool,
        cancel: Arc<AtomicBool>,
    ) -> RefreshResult {
        let base = RefreshResult {
            profile_id: profile_id.clone(),
            source_id: source_id.clone(),
            status: RefreshStatus::Cancelled,
            summary: None,
            config_revision: 0,
        };
        if self.stopped.load(Ordering::SeqCst) || cancel.load(Ordering::SeqCst) {
            return base;
        }
        let core = self.core.clone();
        let p = profile_id.clone();
        let s = source_id.clone();
        let snapshot = tokio::task::spawn_blocking(move || {
            let manager = core.blocking_lock();
            let source = manager
                .subscription_list(&p)?
                .into_iter()
                .find(|x| x.id == s)
                .ok_or_else(|| {
                    crate::error::CommandError::NotFound("subscription source not found".into())
                })?;
            let proxy = manager.proxy_for_traffic(super::network_policy::TRAFFIC_SUBSCRIPTION);
            Ok::<_, crate::error::CommandError>((source, proxy))
        })
        .await;
        let (source, proxy) = match snapshot {
            Ok(Ok(value)) => value,
            _ => {
                return RefreshResult {
                    status: RefreshStatus::Stale,
                    ..base
                }
            }
        };
        if !source.settings.enabled {
            return RefreshResult {
                status: RefreshStatus::Paused,
                ..base
            };
        }
        let key = (profile_id.clone(), source_id.clone());
        let (flight, owner) = {
            let mut flights = self.flights.lock().unwrap();
            if let Some(f) = flights
                .get(&key)
                .filter(|f| f.generation == source.generation)
            {
                (f.clone(), false)
            } else {
                let f = Arc::new(Flight {
                    generation: source.generation,
                    sequence: self.sequence.fetch_add(1, Ordering::SeqCst) + 1,
                    done: Notify::new(),
                    result: Mutex::new(None),
                });
                self.active.fetch_add(1, Ordering::SeqCst);
                flights.insert(key.clone(), f.clone());
                (f, true)
            }
        };
        if !owner {
            loop {
                let notified = flight.done.notified();
                if let Some(result) = flight.result.lock().unwrap().clone() {
                    return result;
                }
                notified.await;
            }
        }
        // A caller dropping its future cannot strand single-flight or skip cleanup.
        let workers = self.clone();
        let task_flight = flight.clone();
        tokio::spawn(async move {
            let result = workers
                .run(
                    &key,
                    &source,
                    proxy,
                    background,
                    &cancel,
                    &task_flight,
                    base,
                )
                .await;
            *task_flight.result.lock().unwrap() = Some(result);
            {
                let mut flights = workers.flights.lock().unwrap();
                if flights
                    .get(&key)
                    .is_some_and(|f| f.sequence == task_flight.sequence)
                {
                    flights.remove(&key);
                }
            }
            task_flight.done.notify_waiters();
            task_flight.done.notify_one();
            workers.active.fetch_sub(1, Ordering::SeqCst);
            workers.idle.notify_waiters();
        });
        loop {
            let notified = flight.done.notified();
            if let Some(result) = flight.result.lock().unwrap().clone() {
                return result;
            }
            notified.await;
        }
    }
    async fn run(
        self: &Arc<Self>,
        key: &Key,
        source: &SubscriptionSource,
        proxy: Result<Option<String>, crate::error::CommandError>,
        background: bool,
        cancel: &Arc<AtomicBool>,
        flight: &Arc<Flight>,
        base: RefreshResult,
    ) -> RefreshResult {
        let _permit = match self.pool.clone().acquire_owned().await {
            Ok(p) => p,
            Err(_) => return base,
        };
        if self.stopped.load(Ordering::SeqCst) || cancel.load(Ordering::SeqCst) {
            return base;
        }
        let url = source.settings.url.clone();
        let fetched = tokio::task::spawn_blocking(move || {
            let pair = proxy
                .map_err(|_| "selected download route is not ready".to_string())
                .and_then(|proxy| SubscriptionManager::fetch_url(&url, proxy.as_deref()));
            let transient = pair.is_err();
            (
                pair.and_then(|(body, headers)| {
                    SubscriptionManager::stage(&body, headers).map_err(|e| e.to_string())
                }),
                transient,
            )
        })
        .await;
        let (staged, transient) = match fetched {
            Ok(value) => value,
            Err(_) => (Err("subscription worker failed".into()), true),
        };
        let workers = self.clone();
        let key_copy = key.clone();
        let source = source.clone();
        let cancel = cancel.clone();
        let seq = flight.sequence;
        let result = tokio::task::spawn_blocking(move || {
            let mut manager = workers.core.blocking_lock();
            if workers.stopped.load(Ordering::SeqCst) || cancel.load(Ordering::SeqCst) {
                return base;
            }
            if workers
                .flights
                .lock()
                .unwrap()
                .get(&key_copy)
                .is_none_or(|f| f.sequence != seq)
            {
                return RefreshResult {
                    status: RefreshStatus::Stale,
                    ..base
                };
            }
            let summary = manager.apply_sync_result(&key_copy.0, &source, staged, background);
            let revision = manager.saved_revision(&key_copy.0).unwrap_or(0);
            match summary {
                Ok(summary) => {
                    let failed = !summary.errors.is_empty();
                    let mut retries = workers.retries.lock().unwrap();
                    if failed {
                        let failures = retries
                            .get(&key_copy)
                            .filter(|r| r.generation == source.generation)
                            .map_or(1, |r| r.failures.saturating_add(1));
                        let delay = if transient {
                            retry_delay_ms(failures)
                        } else {
                            i64::from(source.settings.update_interval_hours) * 3_600_000
                        };
                        retries.insert(
                            key_copy,
                            Retry {
                                generation: source.generation,
                                failures,
                                next_at: super::constants::unix_millis().saturating_add(delay),
                            },
                        );
                    } else {
                        retries.remove(&key_copy);
                    }
                    RefreshResult {
                        status: if failed {
                            RefreshStatus::Failed
                        } else {
                            RefreshStatus::Accepted
                        },
                        summary: Some(summary),
                        config_revision: revision,
                        ..base
                    }
                }
                Err(_) => RefreshResult {
                    status: RefreshStatus::Stale,
                    config_revision: revision,
                    ..base
                },
            }
        })
        .await;
        result.unwrap_or_else(|_| RefreshResult {
            profile_id: key.0.clone(),
            source_id: key.1.clone(),
            status: RefreshStatus::Failed,
            summary: None,
            config_revision: 0,
        })
    }
    pub async fn refresh_all(
        self: &Arc<Self>,
        profile_id: String,
        source_ids: Vec<String>,
        background: bool,
        cancel: Arc<AtomicBool>,
    ) -> Vec<RefreshResult> {
        let mut tasks = tokio::task::JoinSet::new();
        for source_id in source_ids {
            let w = self.clone();
            let p = profile_id.clone();
            let c = cancel.clone();
            tasks.spawn(async move { w.refresh(p, source_id, background, c).await });
        }
        let mut results = Vec::new();
        while let Some(result) = tasks.join_next().await {
            if let Ok(result) = result {
                results.push(result);
            }
        }
        results.sort_by(|a, b| a.source_id.cmp(&b.source_id));
        results
    }
    pub fn is_due_at(&self, profile_id: &str, source: &SubscriptionSource, now: i64) -> bool {
        if !source.settings.enabled || !source.settings.auto_update {
            return false;
        }
        if let Some(retry) = self
            .retries
            .lock()
            .unwrap()
            .get(&(profile_id.into(), source.id.clone()))
            .filter(|r| r.generation == source.generation)
        {
            return now >= retry.next_at;
        }
        source.last_success_at.is_none_or(|at| {
            now.saturating_sub(at) >= i64::from(source.settings.update_interval_hours) * 3_600_000
        })
    }
    pub async fn shutdown(&self) {
        // Linearize shutdown with the commit lock; already-committed work is done.
        {
            let _guard = self.core.lock().await;
            self.stopped.store(true, Ordering::SeqCst);
            self.pool.close();
        }
        loop {
            let notified = self.idle.notified();
            if self.active.load(Ordering::SeqCst) == 0 {
                break;
            }
            notified.await;
        }
    }
}
pub fn retry_delay_ms(failures: u32) -> i64 {
    30_000i64
        .saturating_mul(1i64 << failures.saturating_sub(1).min(7))
        .min(3_600_000)
}
