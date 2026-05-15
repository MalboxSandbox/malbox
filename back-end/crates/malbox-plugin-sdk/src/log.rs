//! Log capture and streaming for plugins.
//!
//! [`LogBus`] is a bounded ring-buffer that collects log entries from a
//! `tracing` subscriber layer and exposes them for streaming back to the
//! daemon over gRPC. The tracing layer lives in [`tracing_layer`].

mod tracing_layer;

pub use tracing_layer::GuestLogLayer;

use std::collections::{HashMap, VecDeque};
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::PathBuf;
use std::sync::Mutex;
use tokio::sync::Notify;

use serde::{Deserialize, Serialize};

/// Severity level for a log entry.
#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LogLevel {
    /// Verbose trace-level output (usually disabled in production).
    Trace = 0,
    /// Diagnostic messages for development.
    Debug = 1,
    /// Normal operational events.
    Info = 2,
    /// Something unexpected but recoverable.
    Warn = 3,
    /// A failure that needs attention.
    Error = 4,
}

impl From<&tracing::Level> for LogLevel {
    fn from(level: &tracing::Level) -> Self {
        match *level {
            tracing::Level::TRACE => LogLevel::Trace,
            tracing::Level::DEBUG => LogLevel::Debug,
            tracing::Level::INFO => LogLevel::Info,
            tracing::Level::WARN => LogLevel::Warn,
            tracing::Level::ERROR => LogLevel::Error,
        }
    }
}

/// A single captured log entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    /// Nanosecond timestamp (wall-clock, UNIX epoch).
    pub timestamp_ns: u64,
    /// Severity level.
    pub level: LogLevel,
    /// The tracing target (usually the module path).
    pub target: String,
    /// The formatted log message.
    pub message: String,
    /// Structured key-value fields attached to the tracing event.
    pub fields: HashMap<String, String>,
}

/// Internal shared state for the log bus.
struct LogBusInner {
    buffer: VecDeque<LogEntry>,
    capacity: usize,
    closed: bool,
    /// Optional overflow destination. When `Some`, evictions append to the
    /// file instead of being dropped.
    overflow_path: Option<PathBuf>,
    overflow_writer: Option<BufWriter<File>>,
}

/// A bounded ring-buffer channel for log entries.
///
/// Thread-safe: the [`Mutex`] protects the buffer, and [`Notify`] wakes async
/// consumers. Producers call [`push`](LogBus::push), the gRPC handler calls
/// [`drain`](LogBus::drain) or [`recv`](LogBus::recv).
pub struct LogBus {
    inner: Mutex<LogBusInner>,
    notify: Notify,
}

impl LogBus {
    /// Create a new log bus that buffers up to `capacity` entries.
    pub fn new(capacity: usize) -> Self {
        Self {
            inner: Mutex::new(LogBusInner {
                buffer: VecDeque::with_capacity(capacity),
                capacity,
                closed: false,
                overflow_path: None,
                overflow_writer: None,
            }),
            notify: Notify::new(),
        }
    }

    /// Create a new log bus that spills evicted entries to a disk overflow
    /// file instead of dropping them.
    pub fn with_overflow(capacity: usize, overflow_path: PathBuf) -> Self {
        Self {
            inner: Mutex::new(LogBusInner {
                buffer: VecDeque::with_capacity(capacity),
                capacity,
                closed: false,
                overflow_path: Some(overflow_path),
                overflow_writer: None,
            }),
            notify: Notify::new(),
        }
    }

    /// Push a log entry into the bus. Drops the oldest entry if at capacity.
    /// No-op if the bus is closed.
    pub fn push(&self, entry: LogEntry) {
        let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        if inner.closed {
            return;
        }
        if inner.buffer.len() >= inner.capacity {
            // Evict the oldest entry. Spill to overflow file if configured.
            let evicted = inner.buffer.pop_front().unwrap();
            if inner.overflow_path.is_some()
                && let Err(e) = write_overflow(&mut inner, &evicted)
            {
                // Overflow write failed (disk full, I/O error). Drop the
                // evicted entry and log an error to stderr. This is the
                // only drop path in the overflow-enabled code path.
                eprintln!("malbox: failed to spill log entry to overflow: {e}");
            }
        }
        inner.buffer.push_back(entry);
        drop(inner);
        self.notify.notify_waiters();
    }

    /// Drain all buffered entries and return them.
    pub fn drain(&self) -> Vec<LogEntry> {
        let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        inner.buffer.drain(..).collect()
    }

    /// Drain overflow file + in-memory ring as a single atomic operation,
    /// returning all entries in strict chronological order. The overflow
    /// file is truncated/removed after a successful drain.
    pub fn drain_atomic(&self) -> Vec<LogEntry> {
        let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());

        // 1. Flush the BufWriter so the file reflects the full producer state.
        if let Some(ref mut w) = inner.overflow_writer {
            let _ = w.flush();
        }

        // 2. Read + parse the overflow file into a local vector.
        let mut out: Vec<LogEntry> = Vec::new();
        if let Some(ref path) = inner.overflow_path
            && path.exists()
            && let Ok(file) = File::open(path)
        {
            let reader = BufReader::new(file);
            for line in reader.lines().map_while(std::io::Result::ok) {
                if line.is_empty() {
                    continue;
                }
                if let Ok(entry) = serde_json::from_str::<LogEntry>(&line) {
                    out.push(entry);
                }
            }
        }

        // 3. Drain the in-memory ring.
        out.extend(inner.buffer.drain(..));

        // 4. Truncate / remove the overflow file so subsequent drains don't
        //    re-read the same entries.
        inner.overflow_writer = None;
        if let Some(ref path) = inner.overflow_path {
            let _ = std::fs::remove_file(path);
        }

        out
    }

    /// Wait until at least one entry is available (in ring or overflow) or
    /// the bus is closed, then drain everything atomically in chronological
    /// order. Returns an empty vector only when the bus is closed and empty.
    pub async fn recv_atomic(&self) -> Vec<LogEntry> {
        loop {
            {
                let inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
                let has_ring = !inner.buffer.is_empty();
                let has_overflow = inner
                    .overflow_path
                    .as_ref()
                    .map(|p| p.exists())
                    .unwrap_or(false);
                if has_ring || has_overflow {
                    drop(inner);
                    return self.drain_atomic();
                }
                if inner.closed {
                    return Vec::new();
                }
            }
            self.notify.notified().await;
        }
    }

    /// Asynchronously wait for the next entry. Returns `None` when the bus is
    /// closed **and** empty.
    pub async fn recv(&self) -> Option<LogEntry> {
        loop {
            {
                let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
                if let Some(entry) = inner.buffer.pop_front() {
                    return Some(entry);
                }
                if inner.closed {
                    return None;
                }
            }
            self.notify.notified().await;
        }
    }

    /// Mark the bus as closed and wake all waiters.
    pub fn close(&self) {
        let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        inner.closed = true;
        drop(inner);
        self.notify.notify_waiters();
    }

    /// Create a [`GuestLogLayer`] that pushes into this bus.
    pub fn layer(self: &std::sync::Arc<Self>) -> GuestLogLayer {
        GuestLogLayer::new(std::sync::Arc::clone(self))
    }
}

fn write_overflow(inner: &mut LogBusInner, entry: &LogEntry) -> std::io::Result<()> {
    let path = inner
        .overflow_path
        .as_ref()
        .expect("overflow_path must be set");
    if inner.overflow_writer.is_none() {
        let file = OpenOptions::new().create(true).append(true).open(path)?;
        inner.overflow_writer = Some(BufWriter::new(file));
    }
    let writer = inner.overflow_writer.as_mut().unwrap();
    let line = serde_json::to_string(entry)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    writeln!(writer, "{line}")?;
    writer.flush()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    fn make_entry(msg: &str, level: LogLevel) -> LogEntry {
        LogEntry {
            timestamp_ns: 0,
            level,
            target: "test".to_string(),
            message: msg.to_string(),
            fields: HashMap::new(),
        }
    }

    #[test]
    fn log_bus_push_and_drain() {
        let bus = LogBus::new(10);
        bus.push(make_entry("a", LogLevel::Info));
        bus.push(make_entry("b", LogLevel::Debug));
        bus.push(make_entry("c", LogLevel::Warn));

        let entries = bus.drain();
        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0].message, "a");
        assert_eq!(entries[1].message, "b");
        assert_eq!(entries[2].message, "c");
    }

    #[test]
    fn log_bus_drops_oldest_at_capacity() {
        let bus = LogBus::new(2);
        bus.push(make_entry("1", LogLevel::Info));
        bus.push(make_entry("2", LogLevel::Info));
        bus.push(make_entry("3", LogLevel::Info));
        bus.push(make_entry("4", LogLevel::Info));
        bus.push(make_entry("5", LogLevel::Info));

        let entries = bus.drain();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].message, "4");
        assert_eq!(entries[1].message, "5");
    }

    #[tokio::test]
    async fn log_bus_recv_returns_entries() {
        let bus = Arc::new(LogBus::new(10));
        bus.push(make_entry("hello", LogLevel::Info));

        let entry = bus.recv().await;
        assert!(entry.is_some());
        assert_eq!(entry.unwrap().message, "hello");
    }

    #[tokio::test]
    async fn log_bus_recv_returns_none_when_closed_and_empty() {
        let bus = Arc::new(LogBus::new(10));
        bus.close();

        let entry = bus.recv().await;
        assert!(entry.is_none());
    }

    #[tokio::test]
    async fn log_bus_close_drains_remaining() {
        let bus = Arc::new(LogBus::new(10));
        bus.push(make_entry("leftover", LogLevel::Warn));
        bus.close();

        let entry = bus.recv().await;
        assert!(entry.is_some());
        assert_eq!(entry.unwrap().message, "leftover");

        let entry = bus.recv().await;
        assert!(entry.is_none());
    }

    #[test]
    fn log_bus_push_after_close_is_noop() {
        let bus = LogBus::new(10);
        bus.close();
        bus.push(make_entry("ignored", LogLevel::Info));

        let entries = bus.drain();
        assert!(entries.is_empty());
    }

    #[test]
    fn log_level_from_tracing() {
        assert_eq!(LogLevel::from(&tracing::Level::TRACE), LogLevel::Trace);
        assert_eq!(LogLevel::from(&tracing::Level::DEBUG), LogLevel::Debug);
        assert_eq!(LogLevel::from(&tracing::Level::INFO), LogLevel::Info);
        assert_eq!(LogLevel::from(&tracing::Level::WARN), LogLevel::Warn);
        assert_eq!(LogLevel::from(&tracing::Level::ERROR), LogLevel::Error);
    }

    #[test]
    fn log_bus_spills_to_overflow_file_when_ring_full() {
        let tmp = tempfile::tempdir().unwrap();
        let overflow_path = tmp.path().join("overflow.jsonl");
        let bus = LogBus::with_overflow(2, overflow_path.clone());

        for i in 0..5 {
            bus.push(make_entry(&format!("msg-{i}"), LogLevel::Info));
        }

        // Drain atomically: must return all 5 entries in order.
        let entries = bus.drain_atomic();
        assert_eq!(entries.len(), 5);
        assert_eq!(entries[0].message, "msg-0");
        assert_eq!(entries[4].message, "msg-4");

        // The overflow file should be deleted after a full drain.
        assert!(
            !overflow_path.exists(),
            "overflow file should be cleaned up after drain"
        );
    }

    #[test]
    fn log_bus_without_overflow_path_drops_under_pressure() {
        // Without overflow, the bus drops the oldest entries under pressure.
        let bus = LogBus::new(2);
        for i in 0..5 {
            bus.push(make_entry(&format!("msg-{i}"), LogLevel::Info));
        }
        let entries = bus.drain();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].message, "msg-3");
        assert_eq!(entries[1].message, "msg-4");
    }
}
