use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crate::capture::{CaptureError, CapturedFrame};
use crate::control::CaptureController;

const FRAME_INTERVAL: Duration = Duration::from_micros(
    (wifimic_protocol::SAMPLES_PER_FRAME as u64 * 1_000_000)
        / (wifimic_protocol::SAMPLE_RATE_HZ as u64),
);

trait DiagnosticClock {
    fn now(&mut self) -> Instant;
    fn wait_until(&mut self, deadline: Instant);
    fn unix_micros(&mut self) -> u64;
}

struct SystemClock;

impl DiagnosticClock for SystemClock {
    fn now(&mut self) -> Instant {
        Instant::now()
    }

    fn wait_until(&mut self, deadline: Instant) {
        let now = Instant::now();
        if deadline > now {
            std::thread::sleep(deadline.duration_since(now));
        }
    }

    fn unix_micros(&mut self) -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |duration| {
                u64::try_from(duration.as_micros()).unwrap_or(u64::MAX)
            })
    }
}

/// A diagnostic-only capture source that emits the protocol's deterministic tone.
pub(super) struct SyntheticCapture {
    clock: Box<dyn DiagnosticClock>,
    running: bool,
    next_frame_at: Option<Instant>,
    next_sequence: u32,
}

impl SyntheticCapture {
    pub(super) fn new() -> Self {
        Self::with_clock(Box::new(SystemClock))
    }

    fn with_clock(clock: Box<dyn DiagnosticClock>) -> Self {
        Self {
            clock,
            running: false,
            next_frame_at: None,
            next_sequence: 0,
        }
    }
}

impl CaptureController for SyntheticCapture {
    fn start(&mut self) -> Result<(), CaptureError> {
        self.running = true;
        self.next_frame_at = Some(self.clock.now());
        self.next_sequence = 0;
        Ok(())
    }

    fn stop(&mut self) -> Result<(), CaptureError> {
        self.running = false;
        self.next_frame_at = None;
        Ok(())
    }

    fn read_frame(&mut self) -> Result<CapturedFrame, CaptureError> {
        if !self.running {
            return Err(CaptureError::NotRunning);
        }
        let scheduled_at = self.next_frame_at.unwrap_or_else(|| self.clock.now());
        self.clock.wait_until(scheduled_at);
        let acquired_at = self.clock.now();
        let acquired_at_unix_us = self.clock.unix_micros();
        let sequence = self.next_sequence;
        self.next_sequence = self.next_sequence.wrapping_add(1);
        let next_frame_at = scheduled_at
            .checked_add(FRAME_INTERVAL)
            .map_or(acquired_at, |next| next.max(acquired_at));
        self.next_frame_at = Some(next_frame_at);
        println!("latency_capture sequence={sequence} acquired_at_unix_us={acquired_at_unix_us}");
        Ok(CapturedFrame {
            pcm: wifimic_protocol::latency::deterministic_tone_frame(sequence),
            acquired_at,
        })
    }

    fn set_sequence(&mut self, sequence: u32) {
        self.next_sequence = sequence;
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use wifimic_protocol::latency::deterministic_tone_frame;

    use crate::control::CaptureController;

    use super::{DiagnosticClock, SyntheticCapture};

    struct ManualClock {
        now: Instant,
        unix_micros: u64,
    }

    impl DiagnosticClock for ManualClock {
        fn now(&mut self) -> Instant {
            self.now
        }

        fn wait_until(&mut self, deadline: Instant) {
            let elapsed = deadline.saturating_duration_since(self.now);
            self.now = deadline;
            self.unix_micros = self
                .unix_micros
                .saturating_add(u64::try_from(elapsed.as_micros()).unwrap_or(u64::MAX));
        }

        fn unix_micros(&mut self) -> u64 {
            self.unix_micros
        }
    }

    #[test]
    fn synthetic_capture_emits_tone_frames_at_protocol_cadence() {
        // Given
        let origin = Instant::now();
        let mut capture = SyntheticCapture::with_clock(Box::new(ManualClock {
            now: origin,
            unix_micros: 1_000_000,
        }));
        capture
            .start()
            .expect("synthetic capture starts without parec");
        capture.set_sequence(7);

        // When
        let first = capture
            .read_frame()
            .expect("first synthetic frame must be available");
        let second = capture
            .read_frame()
            .expect("second synthetic frame must be available");

        // Then
        assert_eq!(first.pcm, deterministic_tone_frame(7));
        assert_eq!(second.pcm, deterministic_tone_frame(8));
        assert_eq!(
            second.acquired_at.duration_since(first.acquired_at),
            Duration::from_millis(5)
        );
    }
}
