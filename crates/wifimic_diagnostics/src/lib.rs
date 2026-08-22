//! Typed, metadata-only diagnostics for the WiFi microphone control plane.

mod event;
mod log_sink;
mod sink;
mod types;

pub use event::{Event, EventType};
pub use log_sink::{LogEventSink, WifimicLogSink};
pub use sink::{EventCollector, EventContext, EventSink, RateLimiter};
pub use types::{
    BufferOperation, ConnectionState, ControlMessageKind, ControlRejectionReason, ErrorClass,
    EventRecord, SessionStopReason,
};

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use super::{Event, EventCollector, EventContext, EventType, RateLimiter};

    #[test]
    fn control_plane_events_are_typed_and_never_format_audio_content() {
        // Given
        let events = [
            (
                Event::CaptureRetry {
                    attempt: 2,
                    error_kind: super::ErrorClass::TimedOut,
                    retry_delay_ms: 5_000,
                },
                EventType::CaptureRetry,
            ),
            (
                Event::HeartbeatTimeout {
                    elapsed_since_heartbeat_ms: 30_000,
                },
                EventType::HeartbeatTimeout,
            ),
            (
                Event::SessionStarted { session_id: 7 },
                EventType::SessionStarted,
            ),
            (
                Event::SessionStopped {
                    session_id: 7,
                    reason: super::SessionStopReason::HeartbeatTimeout,
                },
                EventType::SessionStopped,
            ),
        ];

        // When
        for (event, expected_type) in events {
            let record = super::EventRecord::new(12, Some(7), event);
            let formatted = record.to_string();

            // Then
            assert_eq!(event.event_type(), expected_type);
            assert!(!formatted.contains("pcm"));
            assert!(!formatted.contains("payload"));
            assert!(!formatted.contains("samples"));
            assert!(!formatted.contains("audio_content"));
        }
    }

    #[test]
    fn rate_limiter_admits_first_event_and_exact_interval_boundary() {
        // Given
        let origin = Instant::now();
        let mut limiter = RateLimiter::new(Duration::from_millis(100));

        // When / Then
        assert!(limiter.admit(origin));
        assert!(!limiter.admit(origin + Duration::from_millis(99)));
        assert!(limiter.admit(origin + Duration::from_millis(100)));
    }

    #[test]
    fn zero_interval_admits_events_without_sleeping() {
        // Given
        let origin = Instant::now();
        let mut limiter = RateLimiter::new(Duration::ZERO);

        // When / Then
        assert!(limiter.admit(origin));
        assert!(limiter.admit(origin));
    }

    #[test]
    fn heartbeat_timeout_burst_is_bounded_before_reaching_sink() {
        // Given
        let origin = Instant::now();
        let collector = EventCollector::new();
        let context = EventContext::new(origin, collector.clone());
        let mut limiter = RateLimiter::new(Duration::from_secs(1));

        // When
        for elapsed_ms in 0..1_000 {
            let _ = context.emit_rate_limited(
                origin + Duration::from_millis(elapsed_ms),
                Event::HeartbeatTimeout {
                    elapsed_since_heartbeat_ms: 30_000,
                },
                &mut limiter,
            );
        }

        // Then
        assert_eq!(collector.records().len(), 1);
        assert!(collector.records()[0].event.is_heartbeat_timeout());
    }

    #[test]
    fn rate_limiter_reset_allows_a_new_first_event() {
        // Given
        let origin = Instant::now();
        let mut limiter = RateLimiter::new(Duration::from_secs(1));
        assert!(limiter.admit(origin));
        assert!(!limiter.admit(origin));

        // When
        limiter.reset();

        // Then
        assert!(limiter.admit(origin));
    }
}
