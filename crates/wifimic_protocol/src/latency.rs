mod calibration;
mod measurement;

pub use calibration::{
    decode_calibration, encode_calibration, CalibrationError, CalibrationPacket, CalibrationResult,
    CalibrationTracker, CalibrationUpdate, NtpSample, CALIBRATION_PROBE_BYTES,
    CALIBRATION_PROBE_TAG, CALIBRATION_REPLY_BYTES, CALIBRATION_REPLY_TAG,
    CLOCK_INSTABILITY_THRESHOLD_US, MAX_CALIBRATION_ROUND_TRIP_US, RECALIBRATION_INTERVAL,
};
pub use measurement::{
    application_latency_us, deterministic_tone_frame, LatencyStats, CONSERVATIVE_P95_MARGIN_US,
    MEASUREMENT_TONE_AMPLITUDE, MEASUREMENT_TONE_FREQUENCY_HZ,
};

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
