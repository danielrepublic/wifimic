#[cfg(target_os = "windows")]
#[path = "latency_diagnostic_windows.rs"]
mod windows_backend;

use std::time::Duration;

use thiserror::Error;

pub(crate) const DEFAULT_DURATION_SECS: u64 = 300;
pub(crate) const CAPTURE_ENDPOINT: &str = "CABLE Output (VB-Audio Virtual Cable)";
pub(crate) const TONE_WINDOW_SAMPLES: usize = wifimic_protocol::SAMPLES_PER_FRAME;
pub(crate) const ONSET_RMS_THRESHOLD: u64 = 1_000;

#[derive(Debug, Error)]
pub(crate) enum LatencyDiagnosticError {
    #[error("--duration-secs requires a value")]
    MissingDurationValue,
    #[error("invalid --duration-secs value '{value}'")]
    InvalidDurationValue { value: String },
    #[error("--duration-secs must be greater than zero")]
    ZeroDuration,
    #[cfg(target_os = "windows")]
    #[error(transparent)]
    Windows(#[from] windows_backend::LatencyDiagnosticError),
    #[cfg(not(target_os = "windows"))]
    #[error("latency diagnosis is only supported on Windows")]
    UnsupportedPlatform,
}

pub(crate) fn run_latency_diagnostic() -> Result<(), LatencyDiagnosticError> {
    let duration = parse_duration(std::env::args())?;
    #[cfg(target_os = "windows")]
    {
        windows_backend::run(duration).map_err(LatencyDiagnosticError::from)
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = duration;
        Err(LatencyDiagnosticError::UnsupportedPlatform)
    }
}

fn parse_duration(
    mut arguments: impl Iterator<Item = String>,
) -> Result<Duration, LatencyDiagnosticError> {
    let mut duration_secs = DEFAULT_DURATION_SECS;
    while let Some(argument) = arguments.next() {
        if argument != "--duration-secs" {
            continue;
        }
        let value = arguments
            .next()
            .ok_or(LatencyDiagnosticError::MissingDurationValue)?;
        duration_secs = value
            .parse::<u64>()
            .map_err(|_| LatencyDiagnosticError::InvalidDurationValue { value })?;
    }
    if duration_secs == 0 {
        return Err(LatencyDiagnosticError::ZeroDuration);
    }
    Ok(Duration::from_secs(duration_secs))
}

#[must_use]
pub(crate) fn detect_tone_onset(samples: &[i16]) -> Option<usize> {
    samples
        .chunks_exact(TONE_WINDOW_SAMPLES)
        .enumerate()
        .find_map(|(window_index, window)| {
            let energy = window.iter().fold(0_u64, |sum, sample| {
                let amplitude = i64::from(*sample).unsigned_abs();
                sum.saturating_add(amplitude.saturating_mul(amplitude))
            });
            let threshold_energy = ONSET_RMS_THRESHOLD.saturating_mul(ONSET_RMS_THRESHOLD);
            let window_len = u64::try_from(window.len()).unwrap_or(u64::MAX);
            (energy / window_len >= threshold_energy)
                .then_some(window_index.saturating_mul(TONE_WINDOW_SAMPLES))
        })
}

#[must_use]
pub(crate) fn translate_client_timestamp_to_server(client_us: u64, offset_us: i64) -> u64 {
    let translated = i128::from(client_us) + i128::from(offset_us);
    u64::try_from(translated.max(0)).unwrap_or(u64::MAX)
}

#[cfg(test)]
mod tests {
    use super::{detect_tone_onset, translate_client_timestamp_to_server, TONE_WINDOW_SAMPLES};

    #[test]
    fn onset_detector_returns_none_for_silence() {
        // Given
        let samples = vec![0_i16; TONE_WINDOW_SAMPLES * 3];

        // When
        let onset = detect_tone_onset(&samples);

        // Then
        assert_eq!(onset, None);
    }

    #[test]
    fn onset_detector_returns_first_tone_window_after_silence() {
        // Given
        let mut samples = vec![0_i16; TONE_WINDOW_SAMPLES];
        let tone = wifimic_protocol::latency::deterministic_tone_frame(0);
        samples.extend(
            tone.chunks_exact(wifimic_protocol::BYTES_PER_SAMPLE)
                .map(|sample| i16::from_le_bytes([sample[0], sample[1]])),
        );
        samples.extend([0_i16; TONE_WINDOW_SAMPLES]);

        // When
        let onset = detect_tone_onset(&samples);

        // Then
        assert_eq!(onset, Some(TONE_WINDOW_SAMPLES));
    }

    #[test]
    fn onset_detector_is_stable_for_identical_injected_buffers() {
        // Given
        let mut samples = vec![0_i16; TONE_WINDOW_SAMPLES * 2];
        samples.extend([2_000_i16; TONE_WINDOW_SAMPLES]);
        samples.extend([0_i16; TONE_WINDOW_SAMPLES]);

        // When
        let first = detect_tone_onset(&samples);
        let second = detect_tone_onset(&samples);

        // Then
        assert_eq!(first, Some(TONE_WINDOW_SAMPLES * 2));
        assert_eq!(second, first);
    }

    #[test]
    fn client_onset_timestamp_is_translated_to_server_clock() {
        // Given
        let client_timestamp_us = 1_100_000;

        // When
        let server_timestamp_us =
            translate_client_timestamp_to_server(client_timestamp_us, -25_000);

        // Then
        assert_eq!(server_timestamp_us, 1_075_000);
    }
}
