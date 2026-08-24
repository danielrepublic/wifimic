use std::time::Instant;

use super::{
    dispatch_menu_event, render_if_running, tooltip_text, ClientRunState, MenuEvent, TrayControl,
    TrayDispatch,
};

#[test]
fn tooltip_text_joins_app_name_and_version_with_a_space() {
    // Given
    let name = "wifimic-client";
    let version = "v0.1.7";

    // When
    let tooltip = tooltip_text(name, version);

    // Then
    assert_eq!(tooltip, "wifimic-client v0.1.7");
}

#[derive(Debug, PartialEq, Eq)]
struct TestError;

#[derive(Debug, Default)]
struct FakeControl {
    restart_ids: Vec<u64>,
    calls: Vec<&'static str>,
    next_id: u64,
    stop_error: bool,
}

impl TrayControl for FakeControl {
    type Error = TestError;

    fn restart(&mut self, _now: Instant, epoch_ms: u64) -> Result<u64, Self::Error> {
        self.next_id = self.next_id.max(epoch_ms);
        self.restart_ids.push(self.next_id);
        self.calls.push("restart");
        Ok(self.next_id)
    }

    fn stop(&mut self, _now: Instant) -> Result<(), Self::Error> {
        self.calls.push("stop");
        if self.stop_error {
            Err(TestError)
        } else {
            Ok(())
        }
    }

    fn render_ready(&mut self, _now: Instant) -> Result<(), Self::Error> {
        self.calls.push("render");
        Ok(())
    }
}

#[test]
fn restart_menu_event_calls_restart_once_and_returns_fresh_session_id() {
    // Given
    let mut control = FakeControl::default();
    let mut state = ClientRunState::Running;

    // When
    let dispatch = dispatch_menu_event(
        &mut control,
        MenuEvent::restart(),
        Instant::now(),
        1_000,
        &mut state,
    )
    .expect("Restart dispatch must succeed");

    // Then
    assert_eq!(dispatch, TrayDispatch::Restarted { session_id: 1_000 });
    assert_eq!(control.restart_ids, vec![1_000]);
    assert_eq!(control.calls, vec!["restart"]);
    assert_eq!(state, ClientRunState::Running);
}

#[test]
fn exit_menu_event_stops_before_shutdown_and_suppresses_rendering() {
    // Given
    let mut control = FakeControl::default();
    let mut state = ClientRunState::Running;

    // When
    let dispatch = dispatch_menu_event(
        &mut control,
        MenuEvent::exit(),
        Instant::now(),
        1_000,
        &mut state,
    )
    .expect("Exit dispatch must succeed");
    let render = render_if_running(&mut control, Instant::now(), state);

    // Then
    assert_eq!(dispatch, TrayDispatch::ExitRequested);
    assert_eq!(control.calls, vec!["stop"]);
    assert_eq!(state, ClientRunState::ShutdownRequested);
    assert_eq!(render.expect("render suppression must be infallible"), None);
}

#[test]
fn stop_error_is_reported_after_shutdown_is_marked() {
    // Given
    let mut control = FakeControl {
        stop_error: true,
        ..FakeControl::default()
    };
    let mut state = ClientRunState::Running;

    // When
    let result = dispatch_menu_event(
        &mut control,
        MenuEvent::exit(),
        Instant::now(),
        1_000,
        &mut state,
    );

    // Then
    assert_eq!(result, Err(TestError));
    assert_eq!(control.calls, vec!["stop"]);
    assert_eq!(state, ClientRunState::ShutdownRequested);
}

#[test]
fn unknown_and_duplicate_exit_events_schedule_no_control_work() {
    // Given
    let mut control = FakeControl::default();
    let mut state = ClientRunState::Running;

    // When
    let unknown = dispatch_menu_event(
        &mut control,
        MenuEvent::unknown(),
        Instant::now(),
        1_000,
        &mut state,
    )
    .expect("unknown event must be ignored");
    dispatch_menu_event(
        &mut control,
        MenuEvent::exit(),
        Instant::now(),
        1_000,
        &mut state,
    )
    .expect("first Exit dispatch must succeed");
    let duplicate = dispatch_menu_event(
        &mut control,
        MenuEvent::exit(),
        Instant::now(),
        1_000,
        &mut state,
    )
    .expect("duplicate Exit dispatch must be ignored");

    // Then
    assert_eq!(unknown, TrayDispatch::Ignored);
    assert_eq!(duplicate, TrayDispatch::Ignored);
    assert_eq!(control.calls, vec!["stop"]);
}
