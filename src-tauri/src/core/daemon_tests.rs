use super::*;
use tokio::sync::{mpsc, oneshot};

#[tokio::test]
async fn readiness_waits_for_subscription_setup_and_shutdown_cancels_it() {
    let (stop, shutdown) = oneshot::channel();
    let (ready, accepted) = oneshot::channel::<Result<(), ()>>();
    let (events, mut statuses) = mpsc::unbounded_channel();
    let mut accepted = Some(accepted);
    let task = tokio::spawn(async move {
        run_reconnecting(
            || {
                let accepted = accepted.take().unwrap();
                async move { accepted.await.unwrap() }
            },
            |_| std::future::pending(),
            |status| events.send(status).unwrap(),
            shutdown,
            Duration::from_secs(30),
        )
        .await;
    });
    assert_eq!(statuses.recv().await, Some(STATUS_CONNECTING));
    assert!(statuses.try_recv().is_err());
    ready.send(Ok(())).unwrap();
    assert_eq!(statuses.recv().await, Some(STATUS_RUNNING));
    stop.send(()).unwrap();
    tokio::time::timeout(Duration::from_secs(1), task)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(statuses.recv().await, Some(STATUS_STOPPED));
}

#[tokio::test]
async fn subscription_rejection_never_announces_running_and_retries_with_backoff() {
    let (stop, shutdown) = oneshot::channel();
    let (attempts, mut attempted) = mpsc::unbounded_channel();
    let (events, mut statuses) = mpsc::unbounded_channel();
    let delay = Duration::from_millis(40);
    let task = tokio::spawn(async move {
        run_reconnecting(
            || {
                attempts.send(tokio::time::Instant::now()).unwrap();
                async { Err::<(), _>(tonic::Status::unavailable("subscription rejected")) }
            },
            |_| async { panic!("rejected subscriptions must never run") },
            |status| events.send(status).unwrap(),
            shutdown,
            delay,
        )
        .await;
    });
    let first = attempted.recv().await.unwrap();
    let second = tokio::time::timeout(Duration::from_secs(1), attempted.recv())
        .await
        .unwrap()
        .unwrap();
    assert!(second.duration_since(first) >= delay);
    stop.send(()).unwrap();
    tokio::time::timeout(Duration::from_secs(1), task)
        .await
        .unwrap()
        .unwrap();
    let mut received = Vec::new();
    while let Some(status) = statuses.recv().await {
        received.push(status);
    }
    assert!(!received.contains(&STATUS_RUNNING));
    assert_eq!(received.last(), Some(&STATUS_STOPPED));
}

#[tokio::test]
async fn immediate_stream_eof_is_also_backed_off() {
    let (stop, shutdown) = oneshot::channel();
    let (attempts, mut attempted) = mpsc::unbounded_channel();
    let delay = Duration::from_millis(40);
    let task = tokio::spawn(async move {
        run_reconnecting(
            || {
                attempts.send(tokio::time::Instant::now()).unwrap();
                async { Ok::<_, ()>(()) }
            },
            |_| async {},
            |_| {},
            shutdown,
            delay,
        )
        .await;
    });
    let first = attempted.recv().await.unwrap();
    let second = tokio::time::timeout(Duration::from_secs(1), attempted.recv())
        .await
        .unwrap()
        .unwrap();
    assert!(second.duration_since(first) >= delay);
    stop.send(()).unwrap();
    tokio::time::timeout(Duration::from_secs(1), task)
        .await
        .unwrap()
        .unwrap();
}

#[tokio::test]
async fn shutdown_interrupts_pending_subscription_setup() {
    let (stop, shutdown) = oneshot::channel();
    let (events, mut statuses) = mpsc::unbounded_channel();
    let task = tokio::spawn(async move {
        run_reconnecting(
            std::future::pending::<Result<(), ()>>,
            |_| async { panic!("pending subscriptions must never run") },
            |status| events.send(status).unwrap(),
            shutdown,
            Duration::from_secs(30),
        )
        .await;
    });
    assert_eq!(statuses.recv().await, Some(STATUS_CONNECTING));
    stop.send(()).unwrap();
    tokio::time::timeout(Duration::from_secs(1), task)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(statuses.recv().await, Some(STATUS_STOPPED));
}

fn row(id: &str, up: i64, down: i64) -> ConnectionRow {
    ConnectionRow {
        id: id.into(),
        process_path: "/Applications/Browser.app/Contents/MacOS/Browser".into(),
        process_name: "Browser".into(),
        executable_name: "Browser".into(),
        host: "example.test".into(),
        policy: "proxy".into(),
        is_proxy: true,
        uplink_rate: 10,
        downlink_rate: 20,
        uplink_total: up,
        downlink_total: down,
        closed_at: None,
    }
}

#[test]
fn reconnect_snapshot_preserves_totals_without_double_counting() {
    let mut traffic = TrafficState::default();
    traffic.insert(row("live", 100, 200));
    traffic.insert(row("retired", 20, 30));
    traffic.rows.get_mut("retired").unwrap().closed_at = Some(get_current_epoch());
    let before = build_process_stats(&mut traffic).remove(0);
    assert_eq!((before.total_upload, before.total_download), (120, 230));
    assert_eq!(before.closed_connections, 1);

    traffic.disconnect();
    let disconnected = build_process_stats(&mut traffic).remove(0);
    assert_eq!(
        (disconnected.upload_speed, disconnected.download_speed),
        (0, 0)
    );
    assert_eq!(disconnected.total_upload, 120);
    traffic.reset();
    traffic.insert(row("live", 150, 300));
    let after = build_process_stats(&mut traffic).remove(0);
    assert_eq!((after.total_upload, after.total_download), (170, 330));
    assert_eq!((after.today_upload, after.today_download), (170, 330));
    // The same snapshot can be replayed repeatedly.
    traffic.reset();
    traffic.insert(row("live", 150, 300));
    assert_eq!(build_process_stats(&mut traffic)[0].total_upload, 170);
    traffic.reset();
    let empty = build_process_stats(&mut traffic).remove(0);
    assert_eq!(empty.active_connections, 0);
    assert_eq!(empty.total_upload, 170);
    traffic.insert(row("new-session", 5, 10));
    assert_eq!(build_process_stats(&mut traffic)[0].total_upload, 175);
}

#[test]
fn day_rollover_clears_daily_counters_but_preserves_session_totals() {
    let mut traffic = TrafficState::default();
    traffic.insert(row("live", 100, 200));
    traffic.day = get_current_day() - 1;
    traffic.record("live", 3, 4);
    let stats = build_process_stats(&mut traffic).remove(0);
    assert_eq!((stats.total_upload, stats.total_download), (103, 204));
    assert_eq!((stats.today_upload, stats.today_download), (3, 4));
}

fn key() -> AnalyticsKey {
    AnalyticsKey {
        ts_minute: get_current_epoch() / 60 * 60,
        domain: "example.test".into(),
        policy: "proxy".into(),
        process: "Browser".into(),
        is_proxy: true,
    }
}

#[tokio::test]
async fn partial_analytics_batch_is_drained_to_sqlite_once() {
    let buffer = AnalyticsBuffer::default();
    buffer.lock().unwrap().insert(key(), (10, 20));
    let dir = std::env::temp_dir().join(format!("daemon-analytics-{}", uuid::Uuid::new_v4()));
    let db = crate::core::storage::Db::new(dir.join("traffic.db"));
    flush_analytics_with(&buffer, |batch| {
        let db = &db;
        async move { db.record_traffic_batch(&batch).map_err(|e| e.to_string()) }
    })
    .await;
    assert!(buffer.lock().unwrap().is_empty());
    flush_analytics_with(&buffer, |_| async { panic!("already drained") }).await;
    let series = db.query_traffic_analytics(0, false).unwrap().series;
    assert_eq!(series.len(), 1);
    assert_eq!((series[0].up, series[0].down), (10, 20));
    drop(db);
    std::fs::remove_dir_all(dir).unwrap();
}

#[tokio::test]
async fn failed_analytics_flush_merges_with_deltas_received_during_write() {
    let buffer = AnalyticsBuffer::default();
    buffer.lock().unwrap().insert(key(), (10, 20));
    flush_analytics_with(&buffer, |_| async {
        buffer.lock().unwrap().insert(key(), (3, 4));
        Err("SQLite temporarily unavailable".into())
    })
    .await;
    assert_eq!(buffer.lock().unwrap().get(&key()), Some(&(13, 24)));
    flush_analytics_with(&buffer, |batch| async move {
        assert_eq!(batch.len(), 1);
        assert_eq!((batch[0].up_bytes, batch[0].down_bytes), (13, 24));
        Ok(())
    })
    .await;
    assert!(buffer.lock().unwrap().is_empty());
}

#[test]
fn split_reset_snapshot_replay_preserves_totals_and_retires_stale_rows() {
    let mut traffic = TrafficState::default();
    traffic.insert(row("live", 100, 200));
    traffic.disconnect();
    traffic.reset();
    let gap = build_process_stats(&mut traffic).remove(0);
    assert_eq!(gap.active_connections, 0);
    assert_eq!((gap.total_upload, gap.total_download), (100, 200));
    // A second empty reset must not throw away the unreplayed baseline.
    traffic.reset();
    traffic.insert(row("live", 125, 250));
    let replayed = build_process_stats(&mut traffic).remove(0);
    assert_eq!(replayed.active_connections, 1);
    assert_eq!((replayed.total_upload, replayed.total_download), (125, 250));
}
