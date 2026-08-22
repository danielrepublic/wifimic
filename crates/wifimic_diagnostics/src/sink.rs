use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use super::event::Event;
use super::log_sink::WifimicLogSink;
use super::types::EventRecord;

const DEFAULT_RATE_LIMIT_INTERVAL: Duration = Duration::from_secs(1);

/// A sink receives already-structured, metadata-only event records.
pub trait EventSink: Send + Sync {
    fn record(&self, record: EventRecord);
}

/// A reusable in-process event collector for tests and deterministic QA.
#[derive(Debug, Clone)]
pub struct EventCollector {
    records: Arc<Mutex<VecDeque<EventRecord>>>,
}

impl EventCollector {
    /// The maximum number of records retained by one collector.
    pub const MAX_RECORDS: usize = 4_096;

    /// Creates an empty bounded collector.
    #[must_use]
    pub fn new() -> Self {
        Self {
            records: Arc::new(Mutex::new(VecDeque::with_capacity(Self::MAX_RECORDS))),
        }
    }

    /// Returns a snapshot of the retained records in emission order.
    #[must_use]
    pub fn records(&self) -> Vec<EventRecord> {
        match self.records.lock() {
            Ok(records) => records.iter().copied().collect(),
            Err(_) => Vec::new(),
        }
    }
}

impl Default for EventCollector {
    fn default() -> Self {
        Self::new()
    }
}

impl EventSink for EventCollector {
    fn record(&self, record: EventRecord) {
        if let Ok(mut records) = self.records.lock() {
            if records.len() == Self::MAX_RECORDS {
                records.pop_front();
            }
            records.push_back(record);
        }
    }
}

/// Shared clock and sink used by all events in one process.
#[derive(Clone)]
pub struct EventContext {
    origin: Instant,
    session_id: Option<u64>,
    sink: Arc<dyn EventSink>,
}

impl std::fmt::Debug for EventContext {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("EventContext")
            .field("origin", &self.origin)
            .field("session_id", &self.session_id)
            .finish_non_exhaustive()
    }
}

impl EventContext {
    /// Creates a context that emits records to the supplied sink.
    #[must_use]
    pub fn new<S>(origin: Instant, sink: S) -> Self
    where
        S: EventSink + 'static,
    {
        Self {
            origin,
            session_id: None,
            sink: Arc::new(sink),
        }
    }

    /// Creates a context that forwards records to the repository log facade.
    #[must_use]
    pub fn logging(origin: Instant) -> Self {
        Self::new(origin, WifimicLogSink)
    }

    /// Associates future records with one control-plane session.
    #[must_use]
    pub fn with_session_id(mut self, session_id: u64) -> Self {
        self.session_id = Some(session_id);
        self
    }

    /// Returns the monotonic timestamp origin used by this context.
    #[must_use]
    pub const fn origin(&self) -> Instant {
        self.origin
    }

    /// Emits one event record without rate limiting.
    pub fn emit(&self, now: Instant, event: Event) {
        let record = EventRecord::new(
            elapsed_millis(now.saturating_duration_since(self.origin)),
            self.session_id,
            event,
        );
        self.sink.record(record);
    }

    /// Emits one event when the supplied rate limiter admits it.
    pub fn emit_rate_limited(&self, now: Instant, event: Event, limiter: &mut RateLimiter) -> bool {
        if limiter.admit(now) {
            self.emit(now, event);
            true
        } else {
            false
        }
    }
}

/// A deterministic fixed-interval admission gate for high-frequency events.
#[derive(Debug, Clone, Copy)]
pub struct RateLimiter {
    last_emitted_at: Option<Instant>,
    interval: Duration,
}

impl RateLimiter {
    /// Creates a limiter with the supplied minimum time between admitted events.
    #[must_use]
    pub const fn new(interval: Duration) -> Self {
        Self {
            last_emitted_at: None,
            interval,
        }
    }

    /// Creates the default one-event-per-second limiter.
    #[must_use]
    pub const fn standard() -> Self {
        Self::new(DEFAULT_RATE_LIMIT_INTERVAL)
    }

    /// Returns whether an event at `now` may be emitted and records admission time.
    pub fn admit(&mut self, now: Instant) -> bool {
        let admitted = self
            .last_emitted_at
            .is_none_or(|last| now.saturating_duration_since(last) >= self.interval);
        if admitted {
            self.last_emitted_at = Some(now);
        }
        admitted
    }

    /// Clears the admission timestamp so the next event is admitted immediately.
    pub const fn reset(&mut self) {
        self.last_emitted_at = None;
    }
}

fn elapsed_millis(duration: Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}
