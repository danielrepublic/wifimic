use std::cell::RefCell;
use std::collections::VecDeque;
use std::io;
use std::net::{Ipv4Addr, SocketAddr};
use std::rc::Rc;
use std::time::{Duration, Instant};

use super::control::{DatagramTransport, ReceivedDatagram};
use super::{
    calibrate_transport, emit_render_startup_retry_exhausted, retry_bounded, CalibrationCliError,
};
use wifimic_protocol::latency::{CalibrationError, MAX_CALIBRATION_ROUND_TRIP_US};
use wifimic_protocol::{decode_calibration, encode_calibration, CalibrationPacket};

#[derive(Debug)]
struct FakeCalibrationTransport {
    sent: Vec<Vec<u8>>,
    replies_remaining: usize,
}

impl FakeCalibrationTransport {
    fn new(replies_remaining: usize) -> Self {
        Self {
            sent: Vec::new(),
            replies_remaining,
        }
    }
}

impl DatagramTransport for FakeCalibrationTransport {
    fn send_to_peer(&mut self, payload: &[u8]) -> io::Result<()> {
        self.sent.push(payload.to_vec());
        Ok(())
    }

    fn receive_once(&mut self) -> io::Result<Option<ReceivedDatagram>> {
        if self.replies_remaining == 0 {
            return Ok(None);
        }
        self.replies_remaining -= 1;
        let sent = self.sent.last().expect("a probe must precede its reply");
        let CalibrationPacket::Probe {
            sequence,
            t1_client_send_us,
        } = decode_calibration(sent).expect("fake probe must decode")
        else {
            panic!("calibration helper must send a probe");
        };
        let payload = encode_calibration(CalibrationPacket::Reply {
            sequence,
            t1_client_send_us,
            t2_server_receive_us: t1_client_send_us + 5_000,
            t3_server_send_us: t1_client_send_us + 5_000,
        });
        Ok(Some(ReceivedDatagram {
            source: SocketAddr::from((Ipv4Addr::new(192, 168, 0, 210), 69_02)),
            payload,
        }))
    }
}

fn clock(values: &[u64]) -> impl FnMut() -> u64 + '_ {
    let mut values = VecDeque::from(values.to_vec());
    move || values.pop_front().expect("fake clock has a value")
}

#[test]
fn calibration_retries_one_probe_and_uses_the_later_good_sample() {
    // Given
    let mut transport = FakeCalibrationTransport::new(5);
    let clock_values = [
        1_000, 21_001, 3_000, 5_000, 6_000, 8_000, 9_000, 11_000, 12_000, 14_000,
    ];

    // When
    let tracker = calibrate_transport(&mut transport, clock(&clock_values))
        .expect("a later good sample should recover the probe");

    // Then
    assert_eq!(tracker.offset_us(), Some(4_000));
    let sent_timestamps = transport
        .sent
        .iter()
        .map(|payload| decode_calibration(payload).expect("sent probe must decode"))
        .map(|packet| match packet {
            CalibrationPacket::Probe {
                t1_client_send_us, ..
            } => t1_client_send_us,
            CalibrationPacket::Reply { .. } => panic!("sent packet must be a probe"),
        })
        .collect::<Vec<_>>();
    assert_eq!(sent_timestamps, vec![1_000, 3_000, 6_000, 9_000, 12_000]);
}

#[test]
fn calibration_returns_typed_error_after_ten_long_round_trips() {
    // Given
    let mut transport = FakeCalibrationTransport::new(10);
    let clock_values: Vec<u64> = (0_u64..10)
        .flat_map(|attempt| [attempt * 100_000, attempt * 100_000 + 20_001])
        .collect();

    // When
    let result = calibrate_transport(&mut transport, clock(&clock_values));

    // Then
    assert!(matches!(
        result,
        Err(CalibrationCliError::Calibration(
            CalibrationError::RoundTripTooLong {
                round_trip_us: 20_001,
                maximum_us: MAX_CALIBRATION_ROUND_TRIP_US,
            }
        ))
    ));
    assert_eq!(transport.sent.len(), 10);
}

/// A distinguishable final-error value for the retry-bounded tests.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct TestError(u32);

/// A fake monotonic clock: `now()` reads the current instant and
/// `wait(duration)` records the supplied duration, then advances the
/// clock by that exact duration.
struct FakeClock {
    now: Instant,
    waits: Vec<Duration>,
}

impl FakeClock {
    fn new() -> Self {
        Self {
            now: Instant::now(),
            waits: Vec::new(),
        }
    }
}

fn fake_clock() -> (
    Rc<RefCell<FakeClock>>,
    impl FnMut() -> Instant,
    impl FnMut(Duration),
) {
    let clock = Rc::new(RefCell::new(FakeClock::new()));
    let now = {
        let clock = Rc::clone(&clock);
        move || clock.borrow().now
    };
    let wait = {
        let clock = Rc::clone(&clock);
        move |duration| {
            let mut clock = clock.borrow_mut();
            clock.waits.push(duration);
            clock.now += duration;
        }
    };
    (clock, now, wait)
}

/// An `attempt` fake backed by canned outcomes that also records the
/// remaining budget each call received.
struct AttemptFake {
    outcomes: VecDeque<Result<u32, TestError>>,
    received_budgets: Vec<Duration>,
}

impl AttemptFake {
    fn new(outcomes: Vec<Result<u32, TestError>>) -> Self {
        Self {
            outcomes: VecDeque::from(outcomes),
            received_budgets: Vec::new(),
        }
    }

    fn call(&mut self, remaining: Duration) -> Result<u32, TestError> {
        self.received_budgets.push(remaining);
        self.outcomes
            .pop_front()
            .expect("attempt fake has a canned outcome")
    }
}

#[test]
fn retry_bounded_succeeds_on_first_attempt_without_waiting() {
    // Given
    let (clock, now, wait) = fake_clock();
    let mut attempt = AttemptFake::new(vec![Ok(42)]);

    // When
    let (result, attempts, elapsed) = retry_bounded(
        Duration::from_secs(60),
        Duration::from_secs(2),
        now,
        |remaining| attempt.call(remaining),
        wait,
    );

    // Then
    assert_eq!(result, Ok(42));
    assert_eq!(attempts, 1);
    assert_eq!(elapsed, Duration::ZERO);
    assert_eq!(attempt.received_budgets, vec![Duration::from_secs(60)]);
    assert!(clock.borrow().waits.is_empty());
}

#[test]
fn retry_bounded_succeeds_after_bounded_failures_within_budget() {
    // Given
    let (clock, now, wait) = fake_clock();
    let mut attempt = AttemptFake::new(vec![Err(TestError(1)), Err(TestError(2)), Ok(7)]);

    // When
    let (result, attempts, elapsed) = retry_bounded(
        Duration::from_secs(60),
        Duration::from_secs(2),
        now,
        |remaining| attempt.call(remaining),
        wait,
    );

    // Then
    assert_eq!(result, Ok(7));
    assert_eq!(attempts, 3);
    assert_eq!(elapsed, Duration::from_secs(4));
    assert_eq!(
        attempt.received_budgets,
        vec![
            Duration::from_secs(60),
            Duration::from_secs(58),
            Duration::from_secs(56),
        ]
    );
    assert_eq!(
        clock.borrow().waits,
        vec![Duration::from_secs(2), Duration::from_secs(2)]
    );
}

#[test]
fn retry_bounded_exhausts_the_60_second_budget_and_returns_final_error_unchanged() {
    // Given
    let (clock, now, wait) = fake_clock();
    let outcomes = (1_u32..=30).map(|i| Err(TestError(i))).collect();
    let mut attempt = AttemptFake::new(outcomes);

    // When
    let (result, attempts, elapsed) = retry_bounded(
        Duration::from_secs(60),
        Duration::from_secs(2),
        now,
        |remaining| attempt.call(remaining),
        wait,
    );

    // Then
    assert_eq!(result, Err(TestError(30)));
    assert_eq!(attempts, 30);
    assert_eq!(elapsed, Duration::from_secs(60));
    assert_eq!(clock.borrow().waits, vec![Duration::from_secs(2); 30]);
}

#[test]
fn retry_bounded_caps_the_final_wait_at_the_remaining_budget() {
    // Given
    let (clock, now, wait) = fake_clock();
    let mut attempt = AttemptFake::new(vec![
        Err(TestError(1)),
        Err(TestError(2)),
        Err(TestError(3)),
    ]);

    // When
    let (result, attempts, elapsed) = retry_bounded(
        Duration::from_secs(60),
        Duration::from_secs(25),
        now,
        |remaining| attempt.call(remaining),
        wait,
    );

    // Then
    assert_eq!(result, Err(TestError(3)));
    assert_eq!(attempts, 3);
    assert_eq!(elapsed, Duration::from_secs(60));
    assert_eq!(
        clock.borrow().waits,
        vec![
            Duration::from_secs(25),
            Duration::from_secs(25),
            Duration::from_secs(10),
        ]
    );
}

#[test]
fn emit_render_startup_retry_exhausted_records_one_structured_event() {
    // Given
    let collector = wifimic_diagnostics::EventCollector::new();
    let context = wifimic_diagnostics::EventContext::new(Instant::now(), collector.clone());
    let error = super::render::RenderError::EndpointNotFound {
        expected: "x".to_owned(),
        available: Vec::new(),
    };

    // When
    emit_render_startup_retry_exhausted(
        &context,
        Instant::now(),
        30,
        Duration::from_secs(60),
        &error,
    );

    // Then
    let records = collector.records();
    assert_eq!(records.len(), 1);
    assert!(matches!(
        records[0].event,
        wifimic_diagnostics::Event::RenderStartupRetryExhausted {
            attempt_count: 30,
            elapsed_ms: 60_000,
            failure_class: wifimic_diagnostics::RenderStartupFailureClass::EndpointNotFound,
        }
    ));
}

#[test]
fn windows_startup_emits_exhaustion_only_from_the_open_error_arm() {
    // Given
    let source = include_str!("main.rs");
    let run_windows_client = &source[source
        .find("fn run_windows_client(")
        .expect("Windows startup function is present")..];

    // When
    let emission_count = run_windows_client
        .matches("emit_render_startup_retry_exhausted(")
        .count();

    // Then
    assert_eq!(emission_count, 1);
    assert!(run_windows_client
        .contains("Err(error) => {\n            emit_render_startup_retry_exhausted("));
}
