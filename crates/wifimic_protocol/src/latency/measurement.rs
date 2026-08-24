/// Error margin added to the raw P95 for the acceptance decision.
pub const CONSERVATIVE_P95_MARGIN_US: u64 = 25_000;
/// Deterministic measurement tone frequency.
pub const MEASUREMENT_TONE_FREQUENCY_HZ: u32 = 1_000;
/// Deterministic measurement tone peak amplitude.
pub const MEASUREMENT_TONE_AMPLITUDE: i16 = 8_000;

/// Raw percentile values and the conservative P95 acceptance value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LatencyStats {
    pub raw_p50_us: u64,
    pub raw_p95_us: u64,
    pub raw_p99_us: u64,
    pub conservative_p95_us: u64,
}

const PERCENTILE_ROUNDING_OFFSET: u64 = 99;
const PERCENTILE_SCALE: u64 = 100;
const PERCENTILE_MIN_RANK: u64 = 1;
const PERCENTILE_INDEX_OFFSET: u64 = 1;
const P50_RANK: u64 = 50;
const P95_RANK: u64 = 95;
const P99_RANK: u64 = 99;

impl LatencyStats {
    #[must_use]
    pub fn from_microseconds(samples: impl IntoIterator<Item = u64>) -> Self {
        let mut values = samples.into_iter().collect::<Vec<_>>();
        values.sort_unstable();
        let percentile = |percent: u64| {
            if values.is_empty() {
                return 0;
            }
            let rank = (u64::try_from(values.len()).unwrap_or(u64::MAX) * percent)
                .saturating_add(PERCENTILE_ROUNDING_OFFSET)
                / PERCENTILE_SCALE;
            values[usize::try_from(rank.max(PERCENTILE_MIN_RANK) - PERCENTILE_INDEX_OFFSET)
                .unwrap_or(values.len() - 1)]
        };
        let raw_p95_us = percentile(P95_RANK);
        Self {
            raw_p50_us: percentile(P50_RANK),
            raw_p95_us,
            raw_p99_us: percentile(P99_RANK),
            conservative_p95_us: raw_p95_us.saturating_add(CONSERVATIVE_P95_MARGIN_US),
        }
    }
}

/// Converts a client render timestamp into server-clock application latency.
#[must_use]
pub fn application_latency_us(
    capture_server_us: u64,
    render_client_us: u64,
    server_minus_client_offset_us: i64,
) -> u64 {
    let render_server_us = i128::from(render_client_us) + i128::from(server_minus_client_offset_us);
    let latency_us = render_server_us - i128::from(capture_server_us);
    u64::try_from(latency_us.max(0)).unwrap_or(u64::MAX)
}

/// Generates a phase-continuous 1 kHz PCM frame for the measurement loopback.
#[must_use]
pub fn deterministic_tone_frame(sequence: u32) -> [u8; crate::PCM_PAYLOAD_BYTES] {
    let mut pcm = [0_u8; crate::PCM_PAYLOAD_BYTES];
    for (sample_index, sample_bytes) in pcm.chunks_exact_mut(crate::BYTES_PER_SAMPLE).enumerate() {
        let sample_number = usize::try_from(sequence)
            .unwrap_or(usize::MAX)
            .saturating_mul(crate::SAMPLES_PER_FRAME)
            .saturating_add(sample_index);
        let phase = (sample_number as f64)
            * std::f64::consts::TAU
            * f64::from(MEASUREMENT_TONE_FREQUENCY_HZ)
            / f64::from(crate::SAMPLE_RATE_HZ);
        let sample = (phase.sin() * f64::from(MEASUREMENT_TONE_AMPLITUDE)) as i16;
        sample_bytes.copy_from_slice(&sample.to_le_bytes());
    }
    pcm
}
