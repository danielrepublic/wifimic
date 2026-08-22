use std::time::Duration;

use wifimic_diagnostics::{ControlMessageKind, ControlRejectionReason, Event};
use wifimic_protocol::ControlMessage;

use super::{ControlState, CAPTURE_RETRY_INTERVAL, HEARTBEAT_TIMEOUT};

#[path = "control_capture_retry_tests.rs"]
mod capture_retry_tests;
#[path = "control_test_support.rs"]
mod support;

use support::{command, plane, AckSink, FakeCapture};

#[test]
fn control_streams_while_heartbeats_arrive_within_30s() {
    // Given
    let (mut plane, state, _collector, origin) = plane(vec![true]);
    let mut ack_sink = AckSink::default();

    // When
    ack_sink.record(command(
        &mut plane,
        ControlMessage::Start { session_id: 10 },
        origin,
    ));
    for seconds in [29_u64, 58, 87, 116] {
        ack_sink.record(command(
            &mut plane,
            ControlMessage::Heartbeat { session_id: 10 },
            origin + Duration::from_secs(seconds),
        ));
    }

    // Then
    assert_eq!(plane.state(), ControlState::Streaming);
    assert_eq!(FakeCapture::starts(&state), 1);
    assert_eq!(FakeCapture::stops(&state), 0);
    assert_eq!(ack_sink.messages.len(), 5);
    assert!(ack_sink
        .messages
        .iter()
        .all(|ack| matches!(ack, ControlMessage::Ack { session_id: 10, .. })));
}

#[test]
fn control_start_wire_emits_session_started_and_ack() {
    // Given
    let (mut plane, _state, collector, origin) = plane(vec![true]);

    // When
    let ack = command(&mut plane, ControlMessage::Start { session_id: 11 }, origin);

    // Then
    assert_eq!(
        ack,
        Some(ControlMessage::Ack {
            session_id: 11,
            acked_kind: wifimic_protocol::START_TAG,
        })
    );
    assert!(collector
        .records()
        .iter()
        .any(|record| matches!(record.event, Event::SessionStarted { session_id: 11 })));
}

#[test]
fn control_stops_capture_after_30s_without_heartbeat() {
    // Given
    let (mut plane, state, collector, origin) = plane(vec![true]);
    let _ = command(&mut plane, ControlMessage::Start { session_id: 20 }, origin);

    // When
    plane
        .advance(origin + HEARTBEAT_TIMEOUT + Duration::from_millis(1))
        .expect("timeout stop must succeed");

    // Then
    assert_eq!(plane.state(), ControlState::Idle);
    assert_eq!(FakeCapture::stops(&state), 1);
    assert!(collector
        .records()
        .iter()
        .any(|record| matches!(record.event, Event::HeartbeatTimeout { .. })));
}

#[test]
fn control_retries_capture_every_5s_and_resumes_same_session() {
    // Given
    let (mut plane, state, collector, origin) = plane(vec![false, true]);
    let start_ack = command(&mut plane, ControlMessage::Start { session_id: 30 }, origin);
    assert_eq!(plane.state(), ControlState::Starting);

    // When
    plane
        .advance(origin + CAPTURE_RETRY_INTERVAL)
        .expect("scheduled capture retry must succeed");

    // Then
    assert_eq!(
        start_ack,
        Some(ControlMessage::Ack {
            session_id: 30,
            acked_kind: wifimic_protocol::START_TAG,
        })
    );
    assert_eq!(plane.state(), ControlState::Streaming);
    assert_eq!(FakeCapture::starts(&state), 2);
    assert!(collector.records().iter().any(|record| matches!(
        record.event,
        Event::CaptureRetry {
            attempt: 1,
            retry_delay_ms: 5_000,
            ..
        }
    )));
}

#[test]
fn control_stop_preserves_session_high_water_mark() {
    // Given
    let (mut plane, state, collector, origin) = plane(vec![true]);
    let _ = command(&mut plane, ControlMessage::Start { session_id: 40 }, origin);

    // When
    let stop_ack = command(
        &mut plane,
        ControlMessage::Stop { session_id: 40 },
        origin + Duration::from_secs(1),
    );
    let stale_start_ack = command(
        &mut plane,
        ControlMessage::Start { session_id: 40 },
        origin + Duration::from_secs(2),
    );

    // Then
    assert_eq!(
        stop_ack,
        Some(ControlMessage::Ack {
            session_id: 40,
            acked_kind: wifimic_protocol::STOP_TAG,
        })
    );
    assert!(stale_start_ack.is_none());
    assert_eq!(plane.state(), ControlState::Idle);
    assert_eq!(plane.last_active_session_id(), Some(40));
    assert_eq!(FakeCapture::starts(&state), 1);
    assert!(collector.records().iter().any(|record| matches!(
        record.event,
        Event::ControlMessageRejected {
            kind: ControlMessageKind::Start,
            reason: ControlRejectionReason::StaleSession,
        }
    )));
}

#[test]
fn control_rejects_stale_or_replayed_session_id() {
    // Given
    let (mut plane, state, _collector, origin) = plane(vec![true]);
    let _ = command(&mut plane, ControlMessage::Start { session_id: 50 }, origin);

    // When
    let stale_start_ack = command(
        &mut plane,
        ControlMessage::Start { session_id: 49 },
        origin + Duration::from_secs(1),
    );

    // Then
    assert!(stale_start_ack.is_none());
    assert_eq!(plane.state(), ControlState::Streaming);
    assert_eq!(plane.last_active_session_id(), Some(50));
    assert_eq!(FakeCapture::starts(&state), 1);
}

#[test]
fn control_rejects_mismatched_heartbeat_and_stop() {
    // Given
    let (mut plane, state, _collector, origin) = plane(vec![true]);
    let _ = command(&mut plane, ControlMessage::Start { session_id: 60 }, origin);

    // When
    let heartbeat_ack = command(
        &mut plane,
        ControlMessage::Heartbeat { session_id: 61 },
        origin + Duration::from_secs(1),
    );
    let stop_ack = command(
        &mut plane,
        ControlMessage::Stop { session_id: 61 },
        origin + Duration::from_secs(2),
    );

    // Then
    assert!(heartbeat_ack.is_none());
    assert!(stop_ack.is_none());
    assert_eq!(plane.state(), ControlState::Streaming);
    assert_eq!(FakeCapture::stops(&state), 0);
}

#[test]
fn control_superseding_start_during_streaming_does_not_restart_capture() {
    // Given
    let (mut plane, state, _collector, origin) = plane(vec![true]);
    let _ = command(&mut plane, ControlMessage::Start { session_id: 70 }, origin);

    // When
    let superseding_ack = command(
        &mut plane,
        ControlMessage::Start { session_id: 71 },
        origin + Duration::from_secs(10),
    );
    let old_heartbeat_ack = command(
        &mut plane,
        ControlMessage::Heartbeat { session_id: 70 },
        origin + Duration::from_secs(20),
    );
    let new_heartbeat_ack = command(
        &mut plane,
        ControlMessage::Heartbeat { session_id: 71 },
        origin + Duration::from_secs(39),
    );

    // Then
    assert_eq!(
        superseding_ack,
        Some(ControlMessage::Ack {
            session_id: 71,
            acked_kind: wifimic_protocol::START_TAG,
        })
    );
    assert!(old_heartbeat_ack.is_none());
    assert_eq!(
        new_heartbeat_ack,
        Some(ControlMessage::Ack {
            session_id: 71,
            acked_kind: wifimic_protocol::HEARTBEAT_TAG,
        })
    );
    assert_eq!(plane.state(), ControlState::Streaming);
    assert_eq!(plane.last_active_session_id(), Some(71));
    assert_eq!(FakeCapture::starts(&state), 1);
    assert_eq!(FakeCapture::stops(&state), 0);
}
