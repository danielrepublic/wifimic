#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

pub mod control;
pub mod jitter;
pub mod logging;
pub mod render;

mod tray;
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
    let (_diagnostics, _startup_rotation) = logging::initialize_diagnostics()?;
    if std::env::args().any(|argument| argument == "--calibrate") {
        run_calibration()?;
        return Ok(());
    }
    #[cfg(target_os = "windows")]
    run_windows_client()?;
    Ok(())
}

const CALIBRATION_PROBE_COUNT: u32 = 4;
const CALIBRATION_READ_TIMEOUT: Duration = Duration::from_secs(1);
#[cfg(target_os = "windows")]
const RECEIVE_POLL_INTERVAL: Duration = Duration::from_millis(1);

fn run_calibration() -> Result<(), CalibrationCliError> {
    use std::net::{Ipv4Addr, SocketAddr};

    use control::DatagramTransport;
    use wifimic_protocol::latency::{CalibrationTracker, NtpSample};
    use wifimic_protocol::{decode_calibration, encode_calibration, CalibrationPacket};

    let mut socket =
        control::UdpClientSocket::bind_at(SocketAddr::from((Ipv4Addr::UNSPECIFIED, 0)))?;
    socket.set_read_timeout(Some(CALIBRATION_READ_TIMEOUT))?;
    let mut tracker = CalibrationTracker::new();
    for sequence in 0..CALIBRATION_PROBE_COUNT {
        let t1_client_send_us = unix_micros();
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
            unix_micros(),
        )
        .calibrate()?;
        let update = tracker.update(result);
        println!(
            "calibration sequence={sequence} round_trip_us={} offset_us={} error_bound_us={} instability_warning={}",
            result.round_trip_us, update.offset_us, update.error_bound_us, update.instability_warning
        );
    }
    Ok(())
}

fn unix_micros() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| {
            u64::try_from(duration.as_micros()).unwrap_or(u64::MAX)
        })
}

#[cfg(target_os = "windows")]
fn run_windows_client() -> Result<(), Box<dyn std::error::Error>> {
    use std::time::{Instant, SystemTime, UNIX_EPOCH};

    use control::{ControlPlane, InboundOutcome, UdpClientSocket};
    use render::{RenderConfig, Renderer};
    use tray::{ClientRunState, TrayDispatch};

    let tray = tray::TrayRuntime::new()?;
    let origin = Instant::now();
    let socket = UdpClientSocket::bind()?;
    socket.set_read_timeout(Some(RECEIVE_POLL_INTERVAL))?;
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
