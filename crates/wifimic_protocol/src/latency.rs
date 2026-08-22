use std::time::Duration;

/// Maximum calibration request round trip accepted as a clock sample.
pub const MAX_CALIBRATION_ROUND_TRIP_US: u64 = 20_000;
/// Offset change that makes consecutive calibration samples unstable.
pub const CLOCK_INSTABILITY_THRESHOLD_US: u64 = 5_000;
/// Interval between production clock calibrations.
pub const RECALIBRATION_INTERVAL: Duration = Duration::from_secs(30);
/// Error margin added to the raw P95 for the acceptance decision.
pub const CONSERVATIVE_P95_MARGIN_US: u64 = 25_000;
/// Wire tag for a client calibration probe.
pub const CALIBRATION_PROBE_TAG: u8 = 0x05;
/// Wire tag for a server calibration reply.
pub const CALIBRATION_REPLY_TAG: u8 = 0x06;
/// Exact probe packet length.
pub const CALIBRATION_PROBE_BYTES: usize = 14;
/// Exact reply packet length.
pub const CALIBRATION_REPLY_BYTES: usize = 30;
const CALIBRATION_PREFIX_BYTES: usize = crate::MESSAGE_PREFIX_BYTES;
const CALIBRATION_SEQUENCE_BYTES: usize = crate::SEQUENCE_BYTES;
const CALIBRATION_TIMESTAMP_BYTES: usize = std::mem::size_of::<u64>();
const CALIBRATION_SEQUENCE_START: usize = CALIBRATION_PREFIX_BYTES;
const CALIBRATION_SEQUENCE_END: usize = CALIBRATION_SEQUENCE_START + CALIBRATION_SEQUENCE_BYTES;
const CALIBRATION_T1_START: usize = CALIBRATION_SEQUENCE_END;
const CALIBRATION_T1_END: usize = CALIBRATION_T1_START + CALIBRATION_TIMESTAMP_BYTES;
const CALIBRATION_T2_START: usize = CALIBRATION_T1_END;
const CALIBRATION_T2_END: usize = CALIBRATION_T2_START + CALIBRATION_TIMESTAMP_BYTES;
const CALIBRATION_T3_START: usize = CALIBRATION_T2_END;
const CALIBRATION_T3_END: usize = CALIBRATION_T3_START + CALIBRATION_TIMESTAMP_BYTES;
/// Deterministic measurement tone frequency.
pub const MEASUREMENT_TONE_FREQUENCY_HZ: u32 = 1_000;
/// Deterministic measurement tone peak amplitude.
pub const MEASUREMENT_TONE_AMPLITUDE: i16 = 8_000;

/// Four timestamps from one NTP-style calibration exchange.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NtpSample {
    pub t1_client_send_us: u64,
    pub t2_server_receive_us: u64,
    pub t3_server_send_us: u64,
    pub t4_client_receive_us: u64,
}

/// One calibration packet carried on the existing UDP transport.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CalibrationPacket {
    Probe {
        sequence: u32,
        t1_client_send_us: u64,
    },
    Reply {
        sequence: u32,
        t1_client_send_us: u64,
        t2_server_receive_us: u64,
        t3_server_send_us: u64,
    },
}

#[must_use]
pub fn encode_calibration(packet: CalibrationPacket) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(match packet {
        CalibrationPacket::Probe { .. } => CALIBRATION_PROBE_BYTES,
        CalibrationPacket::Reply { .. } => CALIBRATION_REPLY_BYTES,
    });
    bytes.push(match packet {
        CalibrationPacket::Probe { .. } => CALIBRATION_PROBE_TAG,
        CalibrationPacket::Reply { .. } => CALIBRATION_REPLY_TAG,
    });
    bytes.push(crate::WIRE_VERSION);
    match packet {
        CalibrationPacket::Probe {
            sequence,
            t1_client_send_us,
        } => {
            bytes.extend_from_slice(&sequence.to_be_bytes());
            bytes.extend_from_slice(&t1_client_send_us.to_be_bytes());
        }
        CalibrationPacket::Reply {
            sequence,
            t1_client_send_us,
            t2_server_receive_us,
            t3_server_send_us,
        } => {
            bytes.extend_from_slice(&sequence.to_be_bytes());
            bytes.extend_from_slice(&t1_client_send_us.to_be_bytes());
            bytes.extend_from_slice(&t2_server_receive_us.to_be_bytes());
            bytes.extend_from_slice(&t3_server_send_us.to_be_bytes());
        }
    }
    bytes
}

pub fn decode_calibration(packet: &[u8]) -> Result<CalibrationPacket, crate::ProtocolError> {
    let Some(&tag) = packet.first() else {
        return Err(crate::ProtocolError::Truncated {
            expected: CALIBRATION_PROBE_BYTES,
            actual: 0,
        });
    };
    let expected = match tag {
        CALIBRATION_PROBE_TAG => CALIBRATION_PROBE_BYTES,
        CALIBRATION_REPLY_TAG => CALIBRATION_REPLY_BYTES,
        _ => return Err(crate::ProtocolError::InvalidTag { actual: tag }),
    };
    if packet.get(1) != Some(&crate::WIRE_VERSION) {
        return Err(crate::ProtocolError::InvalidVersion {
            expected: crate::WIRE_VERSION,
            actual: packet.get(1).copied().unwrap_or_default(),
        });
    }
    if packet.len() < expected {
        return Err(crate::ProtocolError::Truncated {
            expected,
            actual: packet.len(),
        });
    }
    if packet.len() > expected {
        return Err(crate::ProtocolError::InvalidLength {
            expected,
            actual: packet.len(),
        });
    }
    let mut sequence = [0_u8; CALIBRATION_SEQUENCE_BYTES];
    sequence.copy_from_slice(&packet[CALIBRATION_SEQUENCE_START..CALIBRATION_SEQUENCE_END]);
    let mut t1 = [0_u8; CALIBRATION_TIMESTAMP_BYTES];
    t1.copy_from_slice(&packet[CALIBRATION_T1_START..CALIBRATION_T1_END]);
    let sequence = u32::from_be_bytes(sequence);
    let t1_client_send_us = u64::from_be_bytes(t1);
    if tag == CALIBRATION_PROBE_TAG {
        return Ok(CalibrationPacket::Probe {
            sequence,
            t1_client_send_us,
        });
    }
    let mut t2 = [0_u8; CALIBRATION_TIMESTAMP_BYTES];
    let mut t3 = [0_u8; CALIBRATION_TIMESTAMP_BYTES];
    t2.copy_from_slice(&packet[CALIBRATION_T2_START..CALIBRATION_T2_END]);
    t3.copy_from_slice(&packet[CALIBRATION_T3_START..CALIBRATION_T3_END]);
    Ok(CalibrationPacket::Reply {
        sequence,
        t1_client_send_us,
        t2_server_receive_us: u64::from_be_bytes(t2),
        t3_server_send_us: u64::from_be_bytes(t3),
    })
}

impl NtpSample {
    #[must_use]
    pub const fn new(
        t1_client_send_us: u64,
        t2_server_receive_us: u64,
        t3_server_send_us: u64,
        t4_client_receive_us: u64,
    ) -> Self {
        Self {
            t1_client_send_us,
            t2_server_receive_us,
            t3_server_send_us,
            t4_client_receive_us,
        }
    }

    pub fn calibrate(self) -> Result<CalibrationResult, CalibrationError> {
        let round_trip_us = self
            .t4_client_receive_us
            .checked_sub(self.t1_client_send_us)
            .ok_or(CalibrationError::InvalidTimestampOrder)?;
        if round_trip_us > MAX_CALIBRATION_ROUND_TRIP_US {
            return Err(CalibrationError::RoundTripTooLong {
                round_trip_us,
                maximum_us: MAX_CALIBRATION_ROUND_TRIP_US,
            });
        }
        let midpoint_client_us = self.t1_client_send_us.saturating_add(round_trip_us / 2);
        let offset_us = i128::from(self.t2_server_receive_us) - i128::from(midpoint_client_us);
        Ok(CalibrationResult {
            offset_us: i64::try_from(offset_us).unwrap_or_else(|_| {
                if offset_us.is_negative() {
                    i64::MIN
                } else {
                    i64::MAX
                }
            }),
            error_bound_us: round_trip_us / 2,
            round_trip_us,
        })
    }
}

/// A successful calibration sample and its conservative clock error bound.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CalibrationResult {
    pub offset_us: i64,
    pub error_bound_us: u64,
    pub round_trip_us: u64,
}

/// A calibration failure that must not update the active clock offset.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CalibrationError {
    InvalidTimestampOrder,
    RoundTripTooLong { round_trip_us: u64, maximum_us: u64 },
}

impl std::fmt::Display for CalibrationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidTimestampOrder => {
                formatter.write_str("calibration timestamps are out of order")
            }
            Self::RoundTripTooLong {
                round_trip_us,
                maximum_us,
            } => write!(
                formatter,
                "calibration round trip {round_trip_us}us exceeds {maximum_us}us"
            ),
        }
    }
}

impl std::error::Error for CalibrationError {}

/// The result of applying a valid calibration sample.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CalibrationUpdate {
    pub offset_us: i64,
    pub error_bound_us: u64,
    pub instability_warning: bool,
}

/// Tracks the active offset while retaining the previous sample for drift checks.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct CalibrationTracker {
    offset_us: Option<i64>,
}

impl CalibrationTracker {
    #[must_use]
    pub const fn new() -> Self {
        Self { offset_us: None }
    }

    #[must_use]
    pub fn update(&mut self, result: CalibrationResult) -> CalibrationUpdate {
        let instability_warning = self.offset_us.is_some_and(|previous| {
            previous.abs_diff(result.offset_us) > CLOCK_INSTABILITY_THRESHOLD_US
        });
        self.offset_us = Some(result.offset_us);
        CalibrationUpdate {
            offset_us: result.offset_us,
            error_bound_us: result.error_bound_us,
            instability_warning,
        }
    }

    #[must_use]
    pub const fn offset_us(self) -> Option<i64> {
        self.offset_us
    }
}

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

#[cfg(test)]
mod tests {
    use super::{
        CalibrationError, CalibrationTracker, LatencyStats, NtpSample,
        CLOCK_INSTABILITY_THRESHOLD_US, CONSERVATIVE_P95_MARGIN_US, MAX_CALIBRATION_ROUND_TRIP_US,
    };

    #[test]
    fn calibration_rejects_round_trip_over_twenty_ms() {
        let sample = NtpSample::new(1_000_000, 1_008_000, 1_009_000, 1_025_001);

        assert_eq!(
            sample.calibrate(),
            Err(CalibrationError::RoundTripTooLong {
                round_trip_us: 25_001,
                maximum_us: MAX_CALIBRATION_ROUND_TRIP_US,
            })
        );
    }

    #[test]
    fn calibration_reports_offset_and_half_round_trip_error_bound() {
        let sample = NtpSample::new(10_000_000, 10_012_000, 10_013_000, 10_020_000);

        let result = sample.calibrate().expect("sample is within the RTT bound");

        assert_eq!(result.round_trip_us, 20_000);
        assert_eq!(result.offset_us, 2_000);
        assert_eq!(result.error_bound_us, 10_000);
    }

    #[test]
    fn tracker_warns_on_consecutive_offset_instability_and_uses_newer_offset() {
        let mut tracker = CalibrationTracker::new();
        let first = NtpSample::new(1_000_000, 1_005_000, 1_006_000, 1_010_000)
            .calibrate()
            .expect("first sample is valid");
        let second = NtpSample::new(2_000_000, 2_013_000, 2_014_000, 2_010_000)
            .calibrate()
            .expect("second sample is valid");

        assert!(!tracker.update(first).instability_warning);
        let update = tracker.update(second);

        assert!(update.instability_warning);
        assert_eq!(update.offset_us, second.offset_us);
        assert!(second.offset_us.abs_diff(first.offset_us) > CLOCK_INSTABILITY_THRESHOLD_US);
    }

    #[test]
    fn latency_stats_report_raw_percentiles_and_conservative_p95() {
        let stats = LatencyStats::from_microseconds([100_000, 120_000, 140_000, 160_000, 180_000]);

        assert_eq!(stats.raw_p50_us, 140_000);
        assert_eq!(stats.raw_p95_us, 180_000);
        assert_eq!(stats.raw_p99_us, 180_000);
        assert_eq!(
            stats.conservative_p95_us,
            180_000 + CONSERVATIVE_P95_MARGIN_US
        );
    }

    #[test]
    fn application_latency_applies_server_to_client_offset() {
        let latency = super::application_latency_us(1_000_000, 1_152_000, 2_000);

        assert_eq!(latency, 154_000);
    }

    #[test]
    fn calibration_wire_round_trip_preserves_four_timestamp_reply() {
        let packet = super::CalibrationPacket::Reply {
            sequence: 4,
            t1_client_send_us: 10,
            t2_server_receive_us: 12,
            t3_server_send_us: 13,
        };

        assert_eq!(
            super::decode_calibration(&super::encode_calibration(packet)),
            Ok(packet)
        );
    }

    #[test]
    fn deterministic_tone_has_stable_onset_and_phase_across_frames() {
        let first = super::deterministic_tone_frame(0);
        let second = super::deterministic_tone_frame(1);

        assert_eq!(&first[..2], &[0, 0]);
        assert_eq!(&second[..2], &first[..2]);
        assert!(first.chunks_exact(2).any(|sample| sample != [0, 0]));
    }
}
