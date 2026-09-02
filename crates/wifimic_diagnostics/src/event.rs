use std::fmt::{self, Display, Formatter};

use super::types::{
    BufferOperation, ConnectionState, ControlMessageKind, ControlRejectionReason, ErrorClass,
    RenderStartupFailureClass, SessionStopReason,
};

/// The bounded set of diagnostic event classifications shared by both processes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventType {
    SenderSendFailure,
    PacketGap,
    MalformedPacket,
    ReorderedRepair,
    OverflowEviction,
    ConnectionTransition,
    PrefillStart,
    UnderrunBurst,
    RenderEventTimeout,
    JitterBufferLockPoisoned,
    CaptureRetry,
    HeartbeatTimeout,
    SessionStarted,
    SessionStopped,
    ControlMessageRejected,
    ClockInstabilityWarning,
    RenderStartupRetryExhausted,
}

impl Display for EventType {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        let name = match self {
            Self::SenderSendFailure => "sender_send_failure",
            Self::PacketGap => "packet_gap",
            Self::MalformedPacket => "malformed_packet",
            Self::ReorderedRepair => "reordered_repair",
            Self::OverflowEviction => "overflow_eviction",
            Self::ConnectionTransition => "connection_transition",
            Self::PrefillStart => "prefill_start",
            Self::UnderrunBurst => "underrun_burst",
            Self::RenderEventTimeout => "render_event_timeout",
            Self::JitterBufferLockPoisoned => "jitter_buffer_lock_poisoned",
            Self::CaptureRetry => "capture_retry",
            Self::HeartbeatTimeout => "heartbeat_timeout",
            Self::SessionStarted => "session_started",
            Self::SessionStopped => "session_stopped",
            Self::ControlMessageRejected => "control_message_rejected",
            Self::ClockInstabilityWarning => "clock_instability_warning",
            Self::RenderStartupRetryExhausted => "render_startup_retry_exhausted",
        };
        formatter.write_str(name)
    }
}

/// Structured metadata for one bounded diagnostic event.
///
/// The variants intentionally contain counters, sequence numbers, lengths, and
/// classifications only. There is no field capable of carrying PCM or a packet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event {
    SenderSendFailure {
        sequence: u32,
        error_kind: ErrorClass,
        total_failures: u64,
    },
    PacketGap {
        sequence: u32,
        missing_frames: u32,
        total_missing_frames: u64,
    },
    MalformedPacket {
        packet_len_bytes: usize,
        total_malformed_packets: u64,
    },
    ReorderedRepair {
        sequence: u32,
        total_reordered_repairs: u64,
    },
    OverflowEviction {
        sequence: u32,
        total_overflow_evictions: u64,
    },
    ConnectionTransition {
        state: ConnectionState,
        reconnected: bool,
        total_transitions: u64,
    },
    PrefillStart {
        queue_frames: usize,
    },
    UnderrunBurst {
        burst_index: u64,
        underrun_frames: u64,
    },
    RenderEventTimeout {
        wait_timeout_ms: u32,
    },
    JitterBufferLockPoisoned {
        operation: BufferOperation,
    },
    CaptureRetry {
        attempt: u32,
        error_kind: ErrorClass,
        retry_delay_ms: u64,
    },
    HeartbeatTimeout {
        elapsed_since_heartbeat_ms: u64,
    },
    SessionStarted {
        session_id: u64,
    },
    SessionStopped {
        session_id: u64,
        reason: SessionStopReason,
    },
    ControlMessageRejected {
        kind: ControlMessageKind,
        reason: ControlRejectionReason,
    },
    ClockInstabilityWarning {
        previous_offset_us: i64,
        new_offset_us: i64,
    },
    RenderStartupRetryExhausted {
        attempt_count: u32,
        elapsed_ms: u64,
        failure_class: RenderStartupFailureClass,
    },
}

impl Event {
    /// Returns the stable classification for this event.
    #[must_use]
    pub const fn event_type(self) -> EventType {
        match self {
            Self::SenderSendFailure { .. } => EventType::SenderSendFailure,
            Self::PacketGap { .. } => EventType::PacketGap,
            Self::MalformedPacket { .. } => EventType::MalformedPacket,
            Self::ReorderedRepair { .. } => EventType::ReorderedRepair,
            Self::OverflowEviction { .. } => EventType::OverflowEviction,
            Self::ConnectionTransition { .. } => EventType::ConnectionTransition,
            Self::PrefillStart { .. } => EventType::PrefillStart,
            Self::UnderrunBurst { .. } => EventType::UnderrunBurst,
            Self::RenderEventTimeout { .. } => EventType::RenderEventTimeout,
            Self::JitterBufferLockPoisoned { .. } => EventType::JitterBufferLockPoisoned,
            Self::CaptureRetry { .. } => EventType::CaptureRetry,
            Self::HeartbeatTimeout { .. } => EventType::HeartbeatTimeout,
            Self::SessionStarted { .. } => EventType::SessionStarted,
            Self::SessionStopped { .. } => EventType::SessionStopped,
            Self::ControlMessageRejected { .. } => EventType::ControlMessageRejected,
            Self::ClockInstabilityWarning { .. } => EventType::ClockInstabilityWarning,
            Self::RenderStartupRetryExhausted { .. } => EventType::RenderStartupRetryExhausted,
        }
    }

    /// Returns whether this record represents a heartbeat timeout.
    #[must_use]
    pub const fn is_heartbeat_timeout(self) -> bool {
        matches!(self, Self::HeartbeatTimeout { .. })
    }
}

impl Display for Event {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::SenderSendFailure {
                sequence,
                error_kind,
                total_failures,
            } => write!(
                formatter,
                "sequence={sequence} error_kind={error_kind} total_failures={total_failures}"
            ),
            Self::PacketGap {
                sequence,
                missing_frames,
                total_missing_frames,
            } => write!(
                formatter,
                "sequence={sequence} missing_frames={missing_frames} total_missing_frames={total_missing_frames}"
            ),
            Self::MalformedPacket {
                packet_len_bytes,
                total_malformed_packets,
            } => write!(
                formatter,
                "packet_len_bytes={packet_len_bytes} total_malformed_packets={total_malformed_packets}"
            ),
            Self::ReorderedRepair {
                sequence,
                total_reordered_repairs,
            } => write!(
                formatter,
                "sequence={sequence} total_reordered_repairs={total_reordered_repairs}"
            ),
            Self::OverflowEviction {
                sequence,
                total_overflow_evictions,
            } => write!(
                formatter,
                "sequence={sequence} total_overflow_evictions={total_overflow_evictions}"
            ),
            Self::ConnectionTransition {
                state,
                reconnected,
                total_transitions,
            } => write!(
                formatter,
                "state={state} reconnected={reconnected} total_transitions={total_transitions}"
            ),
            Self::PrefillStart { queue_frames } => write!(formatter, "queue_frames={queue_frames}"),
            Self::UnderrunBurst {
                burst_index,
                underrun_frames,
            } => write!(
                formatter,
                "burst_index={burst_index} underrun_frames={underrun_frames}"
            ),
            Self::RenderEventTimeout { wait_timeout_ms } => {
                write!(formatter, "wait_timeout_ms={wait_timeout_ms}")
            }
            Self::JitterBufferLockPoisoned { operation } => {
                write!(formatter, "operation={operation}")
            }
            Self::CaptureRetry {
                attempt,
                error_kind,
                retry_delay_ms,
            } => write!(
                formatter,
                "attempt={attempt} error_kind={error_kind} retry_delay_ms={retry_delay_ms}"
            ),
            Self::HeartbeatTimeout {
                elapsed_since_heartbeat_ms,
            } => write!(
                formatter,
                "elapsed_since_heartbeat_ms={elapsed_since_heartbeat_ms}"
            ),
            Self::SessionStarted { session_id } => write!(formatter, "session_id={session_id}"),
            Self::SessionStopped { session_id, reason } => {
                write!(formatter, "session_id={session_id} reason={reason}")
            }
            Self::ControlMessageRejected { kind, reason } => {
                write!(formatter, "kind={kind} reason={reason}")
            }
            Self::ClockInstabilityWarning {
                previous_offset_us,
                new_offset_us,
            } => write!(
                formatter,
                "previous_offset_us={previous_offset_us} new_offset_us={new_offset_us}"
            ),
            Self::RenderStartupRetryExhausted {
                attempt_count,
                elapsed_ms,
                failure_class,
            } => write!(
                formatter,
                "attempt_count={attempt_count} elapsed_ms={elapsed_ms} failure_class={failure_class}"
            ),
        }
    }
}
