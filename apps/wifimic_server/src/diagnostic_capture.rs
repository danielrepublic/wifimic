use crate::capture::{CaptureError, CaptureHandle, CapturedFrame};
use crate::control::CaptureController;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct LatencyCaptureSample {
    sequence: u32,
    acquired_at_unix_us: u64,
}

fn latency_capture_sample(sequence: u32, frame: &CapturedFrame) -> LatencyCaptureSample {
    LatencyCaptureSample {
        sequence,
        acquired_at_unix_us: frame.acquired_at_unix_us,
    }
}

/// Diagnostic-only adapter that keeps the production CaptureHandle path intact.
pub(super) struct LatencyDiagnosticCapture {
    capture: CaptureHandle,
    sequence: u32,
}

impl LatencyDiagnosticCapture {
    pub(super) fn new() -> Self {
        Self {
            capture: CaptureHandle::new(),
            sequence: 0,
        }
    }

    fn sample(&self, frame: &CapturedFrame) -> LatencyCaptureSample {
        latency_capture_sample(self.sequence, frame)
    }
}

impl CaptureController for LatencyDiagnosticCapture {
    fn start(&mut self) -> Result<(), CaptureError> {
        self.capture.start()
    }

    fn stop(&mut self) -> Result<(), CaptureError> {
        self.capture.stop()
    }

    fn read_frame(&mut self) -> Result<CapturedFrame, CaptureError> {
        let frame = self.capture.read_frame()?;
        let sample = self.sample(&frame);
        println!(
            "latency_capture sequence={} acquired_at_unix_us={}",
            sample.sequence, sample.acquired_at_unix_us
        );
        Ok(frame)
    }

    fn set_sequence(&mut self, sequence: u32) {
        self.sequence = sequence;
    }
}

#[cfg(test)]
mod tests {
    use std::time::Instant;

    use crate::capture::CapturedFrame;

    use super::LatencyDiagnosticCapture;

    #[test]
    fn latency_capture_sample_pairs_control_sequence_with_capture_timestamp() {
        // Given: a frame timestamped by the real capture seam, before UDP send.
        let frame = CapturedFrame {
            pcm: [0; wifimic_protocol::PCM_PAYLOAD_BYTES],
            acquired_at: Instant::now(),
            acquired_at_unix_us: 4_200_000,
        };

        // When: the control-plane sequence is attached for diagnostic output.
        let mut capture = LatencyDiagnosticCapture::new();
        crate::control::CaptureController::set_sequence(&mut capture, 17);

        let sample = capture.sample(&frame);

        // Then: offline correlation uses the unchanged wire sequence and read-boundary time.
        assert_eq!(sample.sequence, 17);
        assert_eq!(sample.acquired_at_unix_us, 4_200_000);
    }
}
