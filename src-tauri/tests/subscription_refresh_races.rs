use std::{
    io::{Read, Write},
    net::TcpListener,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc,
    },
};
use tokio::sync::Mutex;
use zeytun_lib::core::{
    dto::*,
    events::EventBus,
    manager::CoreManager,
    storage::Db,
    subscription_refresh::{RefreshStatus, RefreshWorkers},
};

struct Fixture {
    dir: std::path::PathBuf,
    core: Arc<Mutex<CoreManager>>,
    db: Db,
}
impl Fixture {
    fn new() -> Self {
        let dir = std::env::temp_dir().join(format!("refresh-race-{}", uuid::Uuid::new_v4()));
        let manager = CoreManager::new(dir.clone(), None, Arc::new(EventBus::headless())).unwrap();
        let db = Db::new(dir.join("core/zeytun.db"));
        Self {
            dir,
            core: Arc::new(Mutex::new(manager)),
            db,
        }
    }
    fn source(&self, id: &str, url: &str) {
        let s: SubscriptionSource = serde_json::from_value(serde_json::json!({"id":id,"generation":0,"position":0,"settings":{"name":id,"url":url,"enabled":true,"auto_update":true,"update_interval_hours":24}})).unwrap();
        self.db.create_subscription("default", &s).unwrap();
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.dir).unwrap();
    }
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn refresh_all_reports_every_source_beyond_worker_count_and_cancelled_queue() {
    let f = Fixture::new();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/synthetic", listener.local_addr().unwrap());
    let server = std::thread::spawn(move || {
        for _ in 0..9 {
            let (mut stream, _) = listener.accept().unwrap();
            let mut buf = [0u8; 4096];
            let _ = stream.read(&mut buf);
            let body = "socks5://node.test:1080";
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            )
            .unwrap();
        }
    });
    let ids = (0..9).map(|i| format!("s{i}")).collect::<Vec<_>>();
    for id in &ids {
        f.source(id, &url);
    }
    let workers = Arc::new(RefreshWorkers::new(f.core.clone()));
    let results = workers
        .refresh_all(
            "default".into(),
            ids.clone(),
            false,
            Arc::new(AtomicBool::new(false)),
        )
        .await;
    assert_eq!(
        results
            .iter()
            .map(|r| r.source_id.clone())
            .collect::<Vec<_>>(),
        ids
    );
    assert!(results.iter().all(|r| r.status == RefreshStatus::Accepted));
    let results = workers
        .refresh_all(
            "default".into(),
            ids.clone(),
            false,
            Arc::new(AtomicBool::new(true)),
        )
        .await;
    assert_eq!(results.len(), ids.len());
    assert!(results.iter().all(|r| r.status == RefreshStatus::Cancelled));
    workers.shutdown().await;
    server.join().unwrap();
}

#[test]
fn transient_retry_backoff_is_bounded_not_attempt_limited() {
    use zeytun_lib::core::subscription_refresh::retry_delay_ms;
    assert_eq!(retry_delay_ms(1), 30_000);
    assert_eq!(retry_delay_ms(2), 60_000);
    assert_eq!(retry_delay_ms(1000), 3_600_000);
}
fn gated_server() -> (
    String,
    mpsc::Receiver<()>,
    mpsc::Sender<()>,
    std::thread::JoinHandle<()>,
) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/private-token", listener.local_addr().unwrap());
    let (entered_tx, entered_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let thread = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut bytes = [0u8; 4096];
        let _ = stream.read(&mut bytes);
        entered_tx.send(()).unwrap();
        release_rx.recv().unwrap();
        let body = "socks5://new.test:1080#New";
        write!(
            stream,
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        )
        .unwrap();
    });
    (url, entered_rx, release_tx, thread)
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn url_change_during_fetch_rejects_completion_without_failure_metadata() {
    let f = Fixture::new();
    let (url, entered, release, server) = gated_server();
    f.source("a", &url);
    let workers = Arc::new(RefreshWorkers::new(f.core.clone()));
    let w = workers.clone();
    let task = tokio::spawn(async move {
        w.refresh(
            "default".into(),
            "a".into(),
            false,
            Arc::new(AtomicBool::new(false)),
        )
        .await
    });
    tokio::task::spawn_blocking(move || entered.recv().unwrap())
        .await
        .unwrap();
    assert!(
        f.core.try_lock().is_ok(),
        "network must not hold CoreManager lock"
    );
    let mut settings =
        f.db.list_subscriptions("default")
            .unwrap()
            .remove(0)
            .settings;
    settings.url = "http://127.0.0.1:1/new-token".into();
    f.db.update_subscription("default", "a", &settings).unwrap();
    release.send(()).unwrap();
    let result = task.await.unwrap();
    assert_eq!(result.status, RefreshStatus::Stale);
    let source = f.db.list_subscriptions("default").unwrap().remove(0);
    assert!(source.last_success_at.is_none());
    assert!(source.last_error.is_none());
    server.join().unwrap();
    workers.shutdown().await;
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cancellation_after_fetch_started_prevents_commit_and_shutdown_joins() {
    let f = Fixture::new();
    let (url, entered, release, server) = gated_server();
    f.source("a", &url);
    let workers = Arc::new(RefreshWorkers::new(f.core.clone()));
    let cancel = Arc::new(AtomicBool::new(false));
    let c = cancel.clone();
    let w = workers.clone();
    let task = tokio::spawn(async move { w.refresh("default".into(), "a".into(), false, c).await });
    tokio::task::spawn_blocking(move || entered.recv().unwrap())
        .await
        .unwrap();
    cancel.store(true, Ordering::SeqCst);
    release.send(()).unwrap();
    assert_eq!(task.await.unwrap().status, RefreshStatus::Cancelled);
    assert!(f.db.list_subscriptions("default").unwrap()[0]
        .last_attempt_at
        .is_none());
    workers.shutdown().await;
    server.join().unwrap();
}
