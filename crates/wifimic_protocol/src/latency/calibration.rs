use std::time::Duration;

/// Maximum calibration request round trip accepted as a clock sample.
pub const MAX_CALIBRATION_ROUND_TRIP_US: u64 = 20_000;
/// Offset change that makes consecutive calibration samples unstable.
pub const CLOCK_INSTABILITY_THRESHOLD_US: u64 = 5_000;
/// Interval between production clock calibrations.
pub const RECALIBRATION_INTERVAL: Duration = Duration::from_secs(30);
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
