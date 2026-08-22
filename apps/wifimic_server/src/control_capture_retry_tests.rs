use std::time::Duration;

use wifimic_diagnostics::Event;
use wifimic_protocol::ControlMessage;

use super::support::{command, plane_with_read_results, FakeCapture};
use super::{ControlState, CAPTURE_RETRY_INTERVAL, HEARTBEAT_TIMEOUT};

const STREAMING_FAILURE_SESSION_ID: u64 = 80;
const HEARTBEAT_OFFSET: Duration = Duration::from_secs(10);
const FAILURE_OFFSET: Duration = Duration::from_secs(1);
const TIMEOUT_MARGIN: Duration = Duration::from_millis(1);
const FIRST_AUDIO_SEQUENCE: u32 = 0;

#[test]
fn control_streaming_capture_read_failure_schedules_retry_without_error() {
    // Given
    let (mut plane, state, collector, origin) =
        plane_with_read_results(vec![true, true], vec![false]);
    let _ = command(
        &mut plane,
        ControlMessage::Start {
            session_id: STREAMING_FAILURE_SESSION_ID,
        },
        origin,
    );

    // When
    let result = plane.next_audio_frame(FIRST_AUDIO_SEQUENCE, origin + FAILURE_OFFSET);

    // Then
    assert!(matches!(result, Ok(None)));
    assert_eq!(plane.state(), ControlState::Starting);
    assert_eq!(
        plane.last_active_session_id(),
        Some(STREAMING_FAILURE_SESSION_ID)
    );
    assert_eq!(FakeCapture::starts(&state), 1);
    assert!(collector
        .records()
        .iter()
        .any(|record| matches!(record.event, Event::CaptureRetry { attempt: 1, .. })));
}

#[test]
fn control_retries_capture_read_failure_every_5s_and_preserves_same_session_heartbeat() {
    // Given
    let (mut plane, state, _collector, origin) =
        plane_with_read_results(vec![true, true], vec![false]);
    let _ = command(
        &mut plane,
        ControlMessage::Start {
            session_id: STREAMING_FAILURE_SESSION_ID,
        },
        origin,
    );
    let heartbeat_at = origin + HEARTBEAT_OFFSET;
    let _ = command(
        &mut plane,
        ControlMessage::Heartbeat {
            session_id: STREAMING_FAILURE_SESSION_ID,
        },
        heartbeat_at,
    );
    let failure_at = heartbeat_at + FAILURE_OFFSET;
    assert!(matches!(
        plane.next_audio_frame(FIRST_AUDIO_SEQUENCE, failure_at),
        Ok(None)
    ));

    // When
    plane
        .advance(failure_at + CAPTURE_RETRY_INTERVAL)
        .expect("scheduled streaming capture retry must succeed");

    // Then
    assert_eq!(plane.state(), ControlState::Streaming);
    assert_eq!(
        plane.last_active_session_id(),
        Some(STREAMING_FAILURE_SESSION_ID)
    );
    assert_eq!(FakeCapture::starts(&state), 2);
    plane
        .advance(heartbeat_at + HEARTBEAT_TIMEOUT - TIMEOUT_MARGIN)
        .expect("preserved heartbeat must remain valid before timeout");
    assert_eq!(plane.state(), ControlState::Streaming);
    assert!(matches!(
        plane.next_audio_frame(
            FIRST_AUDIO_SEQUENCE,
            heartbeat_at + HEARTBEAT_TIMEOUT - TIMEOUT_MARGIN,
        ),
        Ok(Some(_))
    ));
    assert_eq!(FakeCapture::stops(&state), 0);
}
