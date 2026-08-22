use std::collections::VecDeque;
use std::io;
use std::net::{Ipv4Addr, SocketAddr};

use super::control::{DatagramTransport, ReceivedDatagram};
use super::{calibrate_transport, CalibrationCliError};
use wifimic_protocol::latency::{CalibrationError, MAX_CALIBRATION_ROUND_TRIP_US};
use wifimic_protocol::{decode_calibration, encode_calibration, CalibrationPacket};

#[derive(Debug)]
struct FakeCalibrationTransport {
    sent: Vec<Vec<u8>>,
    replies_remaining: usize,
}

impl FakeCalibrationTransport {
    fn new(replies_remaining: usize) -> Self {
        Self {
            sent: Vec::new(),
            replies_remaining,
        }
    }
}

impl DatagramTransport for FakeCalibrationTransport {
    fn send_to_peer(&mut self, payload: &[u8]) -> io::Result<()> {
        self.sent.push(payload.to_vec());
        Ok(())
    }

    fn receive_once(&mut self) -> io::Result<Option<ReceivedDatagram>> {
        if self.replies_remaining == 0 {
            return Ok(None);
        }
        self.replies_remaining -= 1;
        let sent = self.sent.last().expect("a probe must precede its reply");
        let CalibrationPacket::Probe {
            sequence,
            t1_client_send_us,
        } = decode_calibration(sent).expect("fake probe must decode")
        else {
            panic!("calibration helper must send a probe");
        };
        let payload = encode_calibration(CalibrationPacket::Reply {
            sequence,
            t1_client_send_us,
            t2_server_receive_us: t1_client_send_us + 5_000,
            t3_server_send_us: t1_client_send_us + 5_000,
        });
        Ok(Some(ReceivedDatagram {
            source: SocketAddr::from((Ipv4Addr::new(192, 168, 0, 210), 69_02)),
            payload,
        }))
    }
}

fn clock(values: &[u64]) -> impl FnMut() -> u64 + '_ {
    let mut values = VecDeque::from(values.to_vec());
    move || values.pop_front().expect("fake clock has a value")
}

#[test]
fn calibration_retries_one_probe_and_uses_the_later_good_sample() {
    // Given
    let mut transport = FakeCalibrationTransport::new(5);
    let clock_values = [
        1_000, 21_001, 3_000, 5_000, 6_000, 8_000, 9_000, 11_000, 12_000, 14_000,
    ];

    // When
    let tracker = calibrate_transport(&mut transport, clock(&clock_values))
        .expect("a later good sample should recover the probe");

    // Then
    assert_eq!(tracker.offset_us(), Some(4_000));
    let sent_timestamps = transport
        .sent
        .iter()
        .map(|payload| decode_calibration(payload).expect("sent probe must decode"))
        .map(|packet| match packet {
            CalibrationPacket::Probe {
                t1_client_send_us, ..
            } => t1_client_send_us,
            CalibrationPacket::Reply { .. } => panic!("sent packet must be a probe"),
        })
        .collect::<Vec<_>>();
    assert_eq!(sent_timestamps, vec![1_000, 3_000, 6_000, 9_000, 12_000]);
}

#[test]
fn calibration_returns_typed_error_after_ten_long_round_trips() {
    // Given
    let mut transport = FakeCalibrationTransport::new(10);
    let clock_values: Vec<u64> = (0_u64..10)
        .flat_map(|attempt| [attempt * 100_000, attempt * 100_000 + 20_001])
        .collect();

    // When
    let result = calibrate_transport(&mut transport, clock(&clock_values));

    // Then
    assert!(matches!(
        result,
        Err(CalibrationCliError::Calibration(
            CalibrationError::RoundTripTooLong {
                round_trip_us: 20_001,
                maximum_us: MAX_CALIBRATION_ROUND_TRIP_US,
            }
        ))
    ));
    assert_eq!(transport.sent.len(), 10);
}
