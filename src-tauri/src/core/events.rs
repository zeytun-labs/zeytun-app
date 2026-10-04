//! User-facing event system.
//!
//! Surfaces the handful of lifecycle moments and critical errors a *user* cares
//! about (core started, proxy listening, profile switched, failures) — as
//! opposed to the firehose of internal log lines the old Logs page showed.
//!
//! Events are kept in a bounded in-memory ring buffer (so a freshly-opened
//! Events view can render recent history) and simultaneously streamed to the
//! frontend via the `core-event` Tauri event so an already-open view updates in
//! real time.

use std::collections::VecDeque;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};

/// The Tauri event name real-time events are emitted on.
pub const EVENT_CHANNEL: &str = "core-event";

/// How many past events the ring buffer retains.
const MAX_EVENTS: usize = 500;

/// Severity / intent of a user-facing event. Drives the frontend's colour and
/// badge treatment.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum EventType {
    Info,
    Warning,
    Error,
}

/// A single user-facing event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    /// Stable unique id (UUID v4) — used as the frontend list key.
    pub id: String,
    /// Creation time, Unix epoch milliseconds.
    pub timestamp: u64,
    #[serde(rename = "type")]
    pub event_type: EventType,
    pub title: String,
    pub message: String,
}

impl Event {
    fn new(event_type: EventType, title: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            timestamp: now_unix_ms(),
            event_type,
            title: title.into(),
            message: message.into(),
        }
    }
}

/// Central sink for user-facing events. Cloneable-by-`Arc`; shared between the
/// core manager (producer) and the `core_event_list` command (reader). Each
/// recorded event is pushed onto the ring buffer *and* emitted to the frontend.
pub struct EventBus {
    app: AppHandle,
    buffer: Mutex<VecDeque<Event>>,
}

impl EventBus {
    pub fn new(app: AppHandle) -> Self {
        Self {
            app,
            buffer: Mutex::new(VecDeque::with_capacity(MAX_EVENTS)),
        }
    }

    /// Record and broadcast an event.
    pub fn record(
        &self,
        event_type: EventType,
        title: impl Into<String>,
        message: impl Into<String>,
    ) {
        let event = Event::new(event_type, title, message);

        if let Ok(mut buffer) = self.buffer.lock() {
            buffer.push_back(event.clone());
            while buffer.len() > MAX_EVENTS {
                buffer.pop_front();
            }
        }

        // Best-effort: a missing/closed webview must never break the core path.
        let _ = self.app.emit(EVENT_CHANNEL, &event);
    }

    pub fn info(&self, title: impl Into<String>, message: impl Into<String>) {
        self.record(EventType::Info, title, message);
    }

    pub fn warning(&self, title: impl Into<String>, message: impl Into<String>) {
        self.record(EventType::Warning, title, message);
    }

    pub fn error(&self, title: impl Into<String>, message: impl Into<String>) {
        self.record(EventType::Error, title, message);
    }

    /// Snapshot of retained events, oldest first.
    pub fn list(&self) -> Vec<Event> {
        self.buffer
            .lock()
            .map(|buffer| buffer.iter().cloned().collect())
            .unwrap_or_default()
    }
}

fn now_unix_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or_default()
}
