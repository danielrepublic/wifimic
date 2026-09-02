#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

pub mod control;
pub mod jitter;
pub mod logging;
pub mod render;

mod cli;
mod dispatch;
mod doctor;
mod handoff;
mod latency_diagnostic;
mod status;
mod tray;
use dispatch::dispatch;
use latency_diagnostic::run_latency_diagnostic;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

#[derive(Debug, thiserror::Error)]
enum CalibrationCliError {
    #[error("calibration reply was filtered by the peer boundary")]
    ReplyFilteredByPeerBoundary,
    #[error("calibration peer returned a probe instead of a reply")]
    PeerReturnedProbe,
    #[error("calibration reply sequence did not match the probe")]
    ReplySequenceMismatch,
    #[error(transparent)]
    Transport(#[from] std::io::Error),
    #[error(transparent)]
    Protocol(#[from] wifimic_protocol::ProtocolError),
    #[error(transparent)]
    Calibration(#[from] wifimic_protocol::latency::CalibrationError),
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    if std::env::args().any(|argument| argument == "--calibrate") {
        let (_diagnostics, _startup_rotation) = logging::initialize_diagnostics()?;
        run_calibration()?;
        return Ok(());
    }
    if std::env::args().any(|argument| argument == "--diagnose-latency") {
        let (_diagnostics, _startup_rotation) = logging::initialize_diagnostics()?;
        run_latency_diagnostic()?;
        return Ok(());
    }
    dispatch(cli::parse_command(std::env::args())?)
}

const CALIBRATION_PROBE_COUNT: u32 = 4;
const MAX_ROUND_TRIP_RETRY_ATTEMPTS: u32 = 10;
const CALIBRATION_READ_TIMEOUT: Duration = Duration::from_secs(1);
/// The minimum bound on the adaptive socket read timeout in the main loop.
///
/// Guards against a zero-duration `set_read_timeout` (which panics) when a
/// control timer or jitter playout deadline is already due.
#[cfg(target_os = "windows")]
const CLIENT_READ_TIMEOUT_FLOOR: Duration = Duration::from_millis(1);
/// The maximum bound on the adaptive socket read timeout in the main loop.
///
/// Bounds tray-menu responsiveness (Restart/Exit) and how quickly a fresh
/// connection is noticed when no control timer is currently pending.
#[cfg(target_os = "windows")]
const CLIENT_READ_TIMEOUT_CEILING: Duration = Duration::from_millis(50);

fn run_calibration() -> Result<(), CalibrationCliError> {
    use std::net::{Ipv4Addr, SocketAddr};

    let mut socket =
        control::UdpClientSocket::bind_at(SocketAddr::from((Ipv4Addr::UNSPECIFIED, 0)))?;
    socket.set_read_timeout(Some(CALIBRATION_READ_TIMEOUT))?;
    calibrate_socket(&mut socket)?;
    Ok(())
}

pub(crate) fn calibrate_socket(
    socket: &mut control::UdpClientSocket,
) -> Result<wifimic_protocol::latency::CalibrationTracker, CalibrationCliError> {
    calibrate_transport(socket, unix_micros)
}

fn calibrate_transport<T, C>(
    socket: &mut T,
    mut now_micros: C,
) -> Result<wifimic_protocol::latency::CalibrationTracker, CalibrationCliError>
where
    T: control::DatagramTransport,
    C: FnMut() -> u64,
{
    use wifimic_protocol::latency::CalibrationTracker;

    let mut tracker = CalibrationTracker::new();
    for sequence in 0..CALIBRATION_PROBE_COUNT {
        let result = calibrate_probe(socket, sequence, &mut now_micros)?;
        let update = tracker.update(result);
        println!(
            "calibration sequence={sequence} round_trip_us={} offset_us={} error_bound_us={} instability_warning={}",
            result.round_trip_us, update.offset_us, update.error_bound_us, update.instability_warning
        );
    }
    Ok(tracker)
}

fn calibrate_probe<T, C>(
    socket: &mut T,
    sequence: u32,
    now_micros: &mut C,
) -> Result<wifimic_protocol::latency::CalibrationResult, CalibrationCliError>
where
    T: control::DatagramTransport,
    C: FnMut() -> u64,
{
    use wifimic_protocol::latency::{CalibrationError, NtpSample};
    use wifimic_protocol::{decode_calibration, encode_calibration, CalibrationPacket};

    let mut attempt = 0;
    loop {
        attempt += 1;
        let t1_client_send_us = now_micros();
        socket.send_to_peer(&encode_calibration(CalibrationPacket::Probe {
            sequence,
            t1_client_send_us,
        }))?;
        let Some(datagram) = socket.receive_once()? else {
            return Err(CalibrationCliError::ReplyFilteredByPeerBoundary);
        };
        let CalibrationPacket::Reply {
            sequence: reply_sequence,
            t1_client_send_us,
            t2_server_receive_us,
            t3_server_send_us,
        } = decode_calibration(&datagram.payload)?
        else {
            return Err(CalibrationCliError::PeerReturnedProbe);
        };
        if reply_sequence != sequence {
            return Err(CalibrationCliError::ReplySequenceMismatch);
        }
        let result = NtpSample::new(
            t1_client_send_us,
            t2_server_receive_us,
            t3_server_send_us,
            now_micros(),
        )
        .calibrate();
        match result {
            Ok(result) => return Ok(result),
            Err(CalibrationError::RoundTripTooLong { .. })
                if attempt < MAX_ROUND_TRIP_RETRY_ATTEMPTS =>
            {
                continue
            }
            Err(error @ CalibrationError::RoundTripTooLong { .. }) => return Err(error.into()),
            Err(CalibrationError::InvalidTimestampOrder) => {
                return Err(CalibrationCliError::Calibration(
                    CalibrationError::InvalidTimestampOrder,
                ));
            }
        }
    }
}

/// Retries `attempt` until the injected monotonic clock reaches
/// `max_elapsed`, waiting at most `interval` between failures. The first
/// attempt is immediate; no real time source or sleeper is used here.
/// Returns the final observed result, attempt count, and elapsed time.
fn retry_bounded<T, E, N, O, W>(
    max_elapsed: Duration,
    interval: Duration,
    mut now: N,
    mut attempt: O,
    mut wait: W,
) -> (Result<T, E>, u32, Duration)
where
    N: FnMut() -> std::time::Instant,
    O: FnMut(Duration) -> Result<T, E>,
    W: FnMut(Duration),
{
    let started_at = now();
    let mut attempts = 0_u32;
    loop {
        attempts += 1;
        let remaining = max_elapsed.saturating_sub(now().saturating_duration_since(started_at));
        match attempt(remaining) {
            Ok(value) => {
                return (
                    Ok(value),
                    attempts,
                    now().saturating_duration_since(started_at),
                );
            }
            Err(error) => {
                let elapsed = now().saturating_duration_since(started_at);
                if elapsed >= max_elapsed {
                    return (Err(error), attempts, elapsed);
                }
                let remaining = max_elapsed.saturating_sub(elapsed);
                wait(interval.min(remaining));
                let elapsed_after_wait = now().saturating_duration_since(started_at);
                if elapsed_after_wait >= max_elapsed {
                    return (Err(error), attempts, elapsed_after_wait);
                }
            }
        }
    }
}

fn unix_micros() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| {
            u64::try_from(duration.as_micros()).unwrap_or(u64::MAX)
        })
}

#[cfg(test)]
#[path = "main_tests.rs"]
mod tests;

#[cfg(target_os = "windows")]
fn run_windows_client() -> Result<(), Box<dyn std::error::Error>> {
    use std::time::{Instant, SystemTime, UNIX_EPOCH};

    use control::{ControlPlane, InboundOutcome, UdpClientSocket};
    use render::{RenderConfig, Renderer};
    use tray::{ClientRunState, TrayDispatch};

    let tray = tray::TrayRuntime::new()?;
    let origin = Instant::now();
    let socket = UdpClientSocket::bind()?;
    let renderer = Renderer::open(RenderConfig::vb_cable())?;
    let mut control = ControlPlane::new(socket, renderer, origin);
    let epoch_ms = || {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| u64::try_from(duration.as_millis()).unwrap_or(u64::MAX))
    };
    control.start(origin, epoch_ms()?)?;
    let mut run_state = ClientRunState::Running;

    loop {
        let now = Instant::now();
        tray::pump_windows_messages();
        if let Some(event) = tray.poll_menu_event() {
            let dispatch =
                tray::dispatch_menu_event(&mut control, event, now, epoch_ms()?, &mut run_state)?;
            if dispatch == TrayDispatch::ExitRequested {
                break;
            }
        }
        // Block only until the next actionable deadline (a control timer or
        // a jitter playout slot), capped so tray interaction stays
        // responsive. This replaces a fixed 1ms poll that woke this thread
        // ~1000 times/second even at full idle, which kept the core out of
        // deep sleep states and contributed to sustained fan noise.
        let wait = control
            .next_wakeup()
            .map(|deadline| deadline.saturating_duration_since(now))
            .unwrap_or(CLIENT_READ_TIMEOUT_CEILING)
            .clamp(CLIENT_READ_TIMEOUT_FLOOR, CLIENT_READ_TIMEOUT_CEILING);
        control.transport().set_read_timeout(Some(wait))?;
        match control.receive_once(now) {
            Ok(Some(InboundOutcome::DroppedUnapprovedSource))
            | Ok(Some(InboundOutcome::IgnoredAck { .. }))
            | Ok(Some(InboundOutcome::IgnoredAudio { .. }))
            | Ok(Some(InboundOutcome::IgnoredControl))
            | Ok(Some(InboundOutcome::StartAck { .. }))
            | Ok(Some(InboundOutcome::HeartbeatAck { .. }))
            | Ok(Some(InboundOutcome::AudioQueued { .. }))
            | Ok(Some(InboundOutcome::Calibrated { .. }))
            | Ok(Some(InboundOutcome::CalibrationRejected))
            | Ok(None) => {}
            Err(control::ControlError::Transport(error))
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock
                ) => {}
            Err(control::ControlError::Protocol(_)) => {}
            Err(error) => return Err(error.into()),
        }
        control.advance(now, epoch_ms()?)?;
        let _ = tray::render_if_running(&mut control, now, run_state)?;
    }

    Ok(())
}
