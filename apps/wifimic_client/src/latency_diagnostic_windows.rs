use std::net::{Ipv4Addr, SocketAddr};
use std::thread::sleep;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use thiserror::Error;
use wifimic_protocol::{AudioFrame, SAMPLE_RATE_HZ};

use super::{
    detect_tone_onset, translate_client_timestamp_to_server, RenderCorrelation, RenderedSequence,
    CAPTURE_ENDPOINT,
};

#[path = "latency_diagnostic_capture.rs"]
mod capture;
use capture::{CaptureError, CaptureStream};

const CONTROL_READ_TIMEOUT: Duration = Duration::from_millis(1);
const SESSION_ACK_TIMEOUT: Duration = Duration::from_secs(20);
const MEASUREMENT_INTERVAL: Duration = Duration::from_secs(1);
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
    #[error("the receive/render pipeline produced no frame for the detected tone")]
    NoRenderedToneFrame,
    #[error("the detected tone had no capture timestamp")]
    MissingToneTimestamp,
}

type ClientControl = crate::control::ControlPlane<
    crate::control::UdpClientSocket,
    SequenceRenderer<crate::render::Renderer>,
>;

struct SequenceRenderer<R> {
    inner: R,
    last_sequence: Option<u32>,
}

impl<R> SequenceRenderer<R> {
    fn new(inner: R) -> Self {
        Self {
            inner,
            last_sequence: None,
        }
    }

    fn last_sequence(&self) -> Option<u32> {
        self.last_sequence
    }
}

impl<R> crate::control::AudioRenderer for SequenceRenderer<R>
where
    R: crate::control::AudioRenderer,
{
    fn render_frame(&mut self, frame: &AudioFrame) -> Result<(), crate::render::RenderError> {
        self.inner.render_frame(frame)?;
        self.last_sequence = Some(frame.sequence);
        Ok(())
    }
}

struct ToneOnset {
    sequence: u32,
    client_onset_us: u64,
    server_onset_us: u64,
}

struct ToneMeasurementContext {
    offset_us: i64,
    baseline_rendered: Option<RenderedSequence>,
}

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
    let renderer = SequenceRenderer::new(crate::render::Renderer::open(
        crate::render::RenderConfig::vb_cable(),
    )?);
    let capture = CaptureStream::open()?;
    let mut control = crate::control::ControlPlane::new(socket, renderer, origin);
    control.start(origin, epoch_millis())?;
    wait_for_session(&mut control, origin + SESSION_ACK_TIMEOUT)?;

    let deadline = Instant::now() + duration;
    let mut next_measurement = Instant::now();
    let mut last_rendered = None;
    let _session_id =
        control
            .accepted_session_id()
            .ok_or(LatencyDiagnosticError::SessionAckTimeout {
                timeout_ms: SESSION_ACK_TIMEOUT.as_millis() as u64,
            })?;
    while Instant::now() < deadline {
        let now = Instant::now();
        if let Some(rendered) = service_control(&mut control, now)? {
            last_rendered = Some(rendered);
        }
        if now >= next_measurement {
            let onset = measure_tone(
                &mut control,
                &capture,
                ToneMeasurementContext {
                    offset_us,
                    baseline_rendered: last_rendered,
                },
            )?;
            last_rendered = None;
            println!(
                "latency_onset sequence={} client_onset_us={} server_onset_us={} clock_offset_us={offset_us}",
                onset.sequence, onset.client_onset_us, onset.server_onset_us
            );
            next_measurement = now + MEASUREMENT_INTERVAL;
        }
        sleep(CONTROL_READ_TIMEOUT);
    }
    control.stop(Instant::now())?;
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
) -> Result<Option<RenderedSequence>, LatencyDiagnosticError> {
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
    let render_outcome = control.render_ready(now)?;
    Ok(match render_outcome {
        crate::control::RenderOutcome::Audio => {
            control
                .renderer()
                .last_sequence()
                .map(|sequence| RenderedSequence {
                    sequence,
                    rendered_at_us: epoch_micros(),
                })
        }
        crate::control::RenderOutcome::Gap | crate::control::RenderOutcome::NotReady => None,
    })
}

fn measure_tone(
    control: &mut ClientControl,
    capture: &CaptureStream,
    context: ToneMeasurementContext,
) -> Result<ToneOnset, LatencyDiagnosticError> {
    capture.discard_available()?;
    let mut samples = Vec::new();
    let mut first_capture_us = None;
    let mut render_correlation = RenderCorrelation::new(context.baseline_rendered);
    let deadline = Instant::now() + TONE_CAPTURE_TIMEOUT;
    loop {
        render_correlation.observe(service_control(control, Instant::now())?);
        for packet in capture.read_available()? {
            first_capture_us.get_or_insert(packet.acquired_at_us);
            samples.extend(packet.samples);
        }
        if let Some(onset_index) = detect_tone_onset(&samples) {
            let first_capture_us =
                first_capture_us.ok_or(LatencyDiagnosticError::MissingToneTimestamp)?;
            let onset_client_us = first_capture_us.saturating_add(
                u64::try_from(onset_index)
                    .unwrap_or(u64::MAX)
                    .saturating_mul(TONE_SAMPLE_TIMESTAMP_SCALE)
                    / u64::from(SAMPLE_RATE_HZ),
            );
            let sequence = render_correlation
                .sequence_for_onset(onset_client_us)
                .ok_or(LatencyDiagnosticError::NoRenderedToneFrame)?;
            return Ok(ToneOnset {
                sequence,
                client_onset_us: onset_client_us,
                server_onset_us: translate_client_timestamp_to_server(
                    onset_client_us,
                    context.offset_us,
                ),
            });
        }
        if Instant::now() >= deadline {
            return Err(LatencyDiagnosticError::ToneOnsetTimeout {
                timeout_ms: TONE_CAPTURE_TIMEOUT.as_millis() as u64,
            });
        }
    }
}

fn epoch_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| {
            u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
        })
}

fn epoch_micros() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| {
            u64::try_from(duration.as_micros()).unwrap_or(u64::MAX)
        })
}

#[cfg(test)]
mod tests {
    use wifimic_protocol::AudioFrame;

    use super::SequenceRenderer;

    #[derive(Default)]
    struct RecordingRenderer {
        sequences: Vec<u32>,
    }

    impl crate::control::AudioRenderer for RecordingRenderer {
        fn render_frame(&mut self, frame: &AudioFrame) -> Result<(), crate::render::RenderError> {
            self.sequences.push(frame.sequence);
            Ok(())
        }
    }

    #[test]
    fn sequence_renderer_observes_frames_that_reach_the_real_renderer_seam() {
        // Given
        let mut renderer = SequenceRenderer::new(RecordingRenderer::default());
        let frame = AudioFrame::new(1, 42, [0; wifimic_protocol::PCM_PAYLOAD_BYTES]);

        // When
        crate::control::AudioRenderer::render_frame(&mut renderer, &frame)
            .expect("recording renderer must accept the frame");

        // Then
        assert_eq!(renderer.last_sequence(), Some(42));
        assert_eq!(renderer.inner.sequences, vec![42]);
    }
}
