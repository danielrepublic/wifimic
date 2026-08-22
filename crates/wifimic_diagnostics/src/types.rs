use std::fmt::{self, Display, Formatter};

use super::event::{Event, EventType};

/// A receiver-local liveness state included in connection transition events.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionState {
    Streaming,
    Disconnected,
}

impl Display for ConnectionState {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        let name = match self {
            Self::Streaming => "streaming",
            Self::Disconnected => "disconnected",
        };
        formatter.write_str(name)
    }
}

/// A safe classification of an I/O failure that contains no endpoint or payload data.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorClass {
    PermissionDenied,
    ConnectionRefused,
    ConnectionReset,
    NotConnected,
    WouldBlock,
    TimedOut,
    Interrupted,
    InvalidInput,
    Other,
}

impl Display for ErrorClass {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        let name = match self {
            Self::PermissionDenied => "permission_denied",
            Self::ConnectionRefused => "connection_refused",
            Self::ConnectionReset => "connection_reset",
            Self::NotConnected => "not_connected",
            Self::WouldBlock => "would_block",
            Self::TimedOut => "timed_out",
            Self::Interrupted => "interrupted",
            Self::InvalidInput => "invalid_input",
            Self::Other => "other",
        };
        formatter.write_str(name)
    }
}

impl From<std::io::ErrorKind> for ErrorClass {
    fn from(kind: std::io::ErrorKind) -> Self {
        match kind {
            std::io::ErrorKind::PermissionDenied => Self::PermissionDenied,
            std::io::ErrorKind::ConnectionRefused => Self::ConnectionRefused,
            std::io::ErrorKind::ConnectionReset => Self::ConnectionReset,
            std::io::ErrorKind::NotConnected => Self::NotConnected,
            std::io::ErrorKind::WouldBlock => Self::WouldBlock,
            std::io::ErrorKind::TimedOut => Self::TimedOut,
            std::io::ErrorKind::Interrupted => Self::Interrupted,
            std::io::ErrorKind::InvalidInput => Self::InvalidInput,
            _ => Self::Other,
        }
    }
}

/// The control message kind that was rejected by a later control-plane state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlMessageKind {
    Start,
    Heartbeat,
    Stop,
}

impl Display for ControlMessageKind {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        let name = match self {
            Self::Start => "start",
            Self::Heartbeat => "heartbeat",
            Self::Stop => "stop",
        };
        formatter.write_str(name)
    }
}

/// The typed reason a control message was rejected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlRejectionReason {
    StaleSession,
    InactiveSession,
    SessionMismatch,
}

impl Display for ControlRejectionReason {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        let name = match self {
            Self::StaleSession => "stale_session",
            Self::InactiveSession => "inactive_session",
            Self::SessionMismatch => "session_mismatch",
        };
        formatter.write_str(name)
    }
}

/// The typed reason a session stopped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionStopReason {
    ExplicitRequest,
    HeartbeatTimeout,
    CaptureFailure,
}

impl Display for SessionStopReason {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        let name = match self {
            Self::ExplicitRequest => "explicit_request",
            Self::HeartbeatTimeout => "heartbeat_timeout",
            Self::CaptureFailure => "capture_failure",
        };
        formatter.write_str(name)
    }
}

/// The lock-protected operation associated with a jitter-buffer event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BufferOperation {
    Insert,
    Remove,
    Clear,
}

impl Display for BufferOperation {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        let name = match self {
            Self::Insert => "insert",
            Self::Remove => "remove",
            Self::Clear => "clear",
        };
        formatter.write_str(name)
    }
}

/// One event record with a process-monotonic elapsed timestamp.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EventRecord {
    pub receiver_elapsed_ms: u64,
    pub session_id: Option<u64>,
    pub event_type: EventType,
    pub event: Event,
}

impl EventRecord {
    /// Creates a metadata-only event record.
    #[must_use]
    pub const fn new(receiver_elapsed_ms: u64, session_id: Option<u64>, event: Event) -> Self {
        Self {
            receiver_elapsed_ms,
            session_id,
            event_type: event.event_type(),
            event,
        }
    }
}

impl Display for EventRecord {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "event={} receiver_elapsed_ms={} session_id=",
            self.event_type, self.receiver_elapsed_ms
        )?;
        match self.session_id {
            Some(session_id) => write!(formatter, "{session_id}")?,
            None => formatter.write_str("none")?,
        }
        write!(formatter, " {}", self.event)
    }
}
