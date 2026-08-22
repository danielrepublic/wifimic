use std::net::{Ipv4Addr, SocketAddr};
use std::thread::sleep;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use thiserror::Error;
use wifimic_protocol::latency::{
    application_latency_us, deterministic_tone_frame, LatencyStats, CONSERVATIVE_P95_MARGIN_US,
};
use wifimic_protocol::{AudioFrame, SAMPLE_RATE_HZ};

use super::{detect_tone_onset, CAPTURE_ENDPOINT};

#[path = "latency_diagnostic_capture.rs"]
mod capture;
use capture::{CaptureError, CaptureStream};

const CONTROL_READ_TIMEOUT: Duration = Duration::from_millis(1);
const SESSION_ACK_TIMEOUT: Duration = Duration::from_secs(20);
const MEASUREMENT_INTERVAL: Duration = Duration::from_secs(1);
const TONE_FRAME_COUNT: u32 = 20;
const TONE_CAPTURE_TIMEOUT: Duration = Duration::from_millis(250);
const TONE_SAMPLE_TIMESTAMP_SCALE: u64 = 1_000_000;

#[derive(Debug, Error)]
pub(crate) enum LatencyDiagnosticError {
    #[error(transparent)]
    Transport(#[from] std::io::Error),
    #[error(transparent)]
    Calibration(#[from] crate::CalibrationCliError),
    #[error(transparent)]
    Control(#[from] crate::control::ControlError),
    #[error(transparent)]
    Capture(#[from] CaptureError),
    #[error(transparent)]
    Render(#[from] crate::render::RenderError),
    #[error("Linux server did not acknowledge the diagnostic Start within {timeout_ms}ms")]
    SessionAckTimeout { timeout_ms: u64 },
    #[error("the capture stream did not detect the measurement tone within {timeout_ms}ms")]
    ToneOnsetTimeout { timeout_ms: u64 },
    #[error("the latency diagnostic collected no measurements")]
    NoMeasurements,
    #[error("system clock is before the Unix epoch")]
    ClockBeforeEpoch,
}

type ClientControl =
    crate::control::ControlPlane<crate::control::UdpClientSocket, crate::render::Renderer>;

pub(super) fn run(duration: Duration) -> Result<(), LatencyDiagnosticError> {
    let mut socket =
        crate::control::UdpClientSocket::bind_at(SocketAddr::from((Ipv4Addr::UNSPECIFIED, 0)))?;
    socket.set_read_timeout(Some(crate::CALIBRATION_READ_TIMEOUT))?;
    let calibration = crate::calibrate_socket(&mut socket)?;
    let offset_us = calibration
        .offset_us()
        .ok_or(LatencyDiagnosticError::NoMeasurements)?;
    socket.set_read_timeout(Some(CONTROL_READ_TIMEOUT))?;

    println!(
        "latency_diagnostic endpoint_render=\"{}\" endpoint_capture=\"{}\" duration_secs={}",
        crate::render::DEFAULT_RENDER_ENDPOINT,
        CAPTURE_ENDPOINT,
        duration.as_secs()
    );
    let origin = Instant::now();
    let renderer = crate::render::Renderer::open(crate::render::RenderConfig::vb_cable())?;
    let capture = CaptureStream::open()?;
    let mut control = crate::control::ControlPlane::new(socket, renderer, origin);
    control.start(origin, epoch_millis())?;
    wait_for_session(&mut control, origin + SESSION_ACK_TIMEOUT)?;

    let deadline = Instant::now() + duration;
    let mut next_measurement = Instant::now();
    let session_id =
        control
            .accepted_session_id()
            .ok_or(LatencyDiagnosticError::SessionAckTimeout {
                timeout_ms: SESSION_ACK_TIMEOUT.as_millis() as u64,
            })?;
    let mut sequence = 0_u32;
    let mut latencies = Vec::new();
    while Instant::now() < deadline {
        let now = Instant::now();
        service_control(&mut control, now)?;
        if now >= next_measurement {
            let latency = measure_tone(&control, &capture, session_id, sequence, offset_us)?;
            println!("latency_sample sequence={sequence} raw_latency_us={latency}");
            latencies.push(latency);
            sequence = sequence.wrapping_add(TONE_FRAME_COUNT);
            next_measurement = now + MEASUREMENT_INTERVAL;
        }
        sleep(CONTROL_READ_TIMEOUT);
    }
    control.stop(Instant::now())?;
    let stats = LatencyStats::from_microseconds(latencies.iter().copied());
    if latencies.is_empty() {
        return Err(LatencyDiagnosticError::NoMeasurements);
    }
    println!(
        "latency_stats raw_p50_us={} raw_p95_us={} raw_p99_us={} conservative_p95_us={} conservative_p95_margin_us={}",
        stats.raw_p50_us,
        stats.raw_p95_us,
        stats.raw_p99_us,
        stats.conservative_p95_us,
        CONSERVATIVE_P95_MARGIN_US
    );
    Ok(())
}

fn wait_for_session(
    control: &mut ClientControl,
    deadline: Instant,
) -> Result<(), LatencyDiagnosticError> {
    while control.accepted_session_id().is_none() {
        let now = Instant::now();
        if now >= deadline {
            return Err(LatencyDiagnosticError::SessionAckTimeout {
                timeout_ms: SESSION_ACK_TIMEOUT.as_millis() as u64,
            });
        }
        service_control(control, now)?;
        sleep(CONTROL_READ_TIMEOUT);
    }
    Ok(())
}

fn service_control(
    control: &mut ClientControl,
    now: Instant,
) -> Result<(), LatencyDiagnosticError> {
    match control.receive_once(now) {
        Ok(_) => {}
        Err(crate::control::ControlError::Transport(error))
            if matches!(
                error.kind(),
                std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock
            ) => {}
        Err(error) => return Err(error.into()),
    }
    control.advance(now, epoch_millis())?;
    Ok(())
}

fn measure_tone(
    control: &ClientControl,
    capture: &CaptureStream,
    session_id: u64,
    sequence: u32,
    offset_us: i64,
) -> Result<u64, LatencyDiagnosticError> {
    capture.discard_available()?;
    let render_client_us = unix_micros()?;
    for frame_sequence in sequence..sequence.saturating_add(TONE_FRAME_COUNT) {
        let frame = AudioFrame::new(
            session_id,
            frame_sequence,
            deterministic_tone_frame(frame_sequence),
        );
        control.renderer().render_frame(&frame)?;
    }
    let mut samples = Vec::new();
    let mut first_capture_us = None;
    let deadline = Instant::now() + TONE_CAPTURE_TIMEOUT;
    loop {
        for packet in capture.read_available()? {
            first_capture_us.get_or_insert(packet.acquired_at_us);
            samples.extend(packet.samples);
        }
        if let Some(onset_index) = detect_tone_onset(&samples) {
            let onset_client_us = first_capture_us.unwrap_or(render_client_us).saturating_add(
                u64::try_from(onset_index)
                    .unwrap_or(u64::MAX)
                    .saturating_mul(TONE_SAMPLE_TIMESTAMP_SCALE)
                    / u64::from(SAMPLE_RATE_HZ),
            );
            let linux_capture_frame_acquisition_us =
                translate_client_anchor_to_linux(render_client_us, offset_us);
            return Ok(application_latency_us(
                linux_capture_frame_acquisition_us,
                onset_client_us,
                offset_us,
            ));
        }
        if Instant::now() >= deadline {
            return Err(LatencyDiagnosticError::ToneOnsetTimeout {
                timeout_ms: TONE_CAPTURE_TIMEOUT.as_millis() as u64,
            });
        }
    }
}

fn translate_client_anchor_to_linux(client_us: u64, offset_us: i64) -> u64 {
    let translated = i128::from(client_us) + i128::from(offset_us);
    u64::try_from(translated.max(0)).unwrap_or(u64::MAX)
}

fn epoch_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| {
            u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
        })
}

fn unix_micros() -> Result<u64, LatencyDiagnosticError> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| u64::try_from(duration.as_micros()).unwrap_or(u64::MAX))
        .map_err(|_| LatencyDiagnosticError::ClockBeforeEpoch)
}
