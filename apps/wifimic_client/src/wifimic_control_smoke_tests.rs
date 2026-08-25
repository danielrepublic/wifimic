use std::io;
use std::net::{Ipv4Addr, SocketAddr, UdpSocket};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use wifimic_protocol::{
    decode_control, encode_audio_frame, encode_control, AudioFrame, ControlMessage,
    PCM_PAYLOAD_BYTES,
};

use super::{SmokeClient, SmokeError};

#[derive(Debug, Clone, Copy)]
enum ResponderMode {
    Matching,
    Missing,
    Mismatched,
    AudioThenMatching,
    AudioOnly,
    AudioThenMismatched,
}

fn spawn_responder(mode: ResponderMode) -> (SocketAddr, JoinHandle<()>) {
    let socket = UdpSocket::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, 0_u16)))
        .expect("test responder must bind");
    let address = socket
        .local_addr()
        .expect("test responder must have an address");
    let handle = thread::spawn(move || {
        socket
            .set_read_timeout(Some(Duration::from_millis(250)))
            .expect("test responder must set a bounded receive timeout");
        let exchanges = match mode {
            ResponderMode::Matching | ResponderMode::AudioThenMatching => 3,
            ResponderMode::Missing
            | ResponderMode::Mismatched
            | ResponderMode::AudioOnly
            | ResponderMode::AudioThenMismatched => 1,
        };
        for _ in 0..exchanges {
            let mut packet = [0_u8; 64];
            let Ok((received, source)) = socket.recv_from(&mut packet) else {
                return;
            };
            let request = decode_control(&packet[..received]).expect("request must decode");
            let (session_id, acked_kind) = match request {
                ControlMessage::Start { session_id } => (session_id, wifimic_protocol::START_TAG),
                ControlMessage::Heartbeat { session_id } => {
                    (session_id, wifimic_protocol::HEARTBEAT_TAG)
                }
                ControlMessage::Stop { session_id } => (session_id, wifimic_protocol::STOP_TAG),
                ControlMessage::Ack { .. } => panic!("smoke client must not send Ack"),
            };
            if matches!(
                mode,
                ResponderMode::AudioThenMatching
                    | ResponderMode::AudioOnly
                    | ResponderMode::AudioThenMismatched
            ) {
                let audio = AudioFrame::new(session_id, 0, [0_u8; PCM_PAYLOAD_BYTES]);
                socket
                    .send_to(&encode_audio_frame(&audio), source)
                    .expect("test responder must send an audio frame");
            }
            let response = match mode {
                ResponderMode::Matching | ResponderMode::AudioThenMatching => {
                    Some(ControlMessage::Ack {
                        session_id,
                        acked_kind,
                    })
                }
                ResponderMode::Missing | ResponderMode::AudioOnly => None,
                ResponderMode::Mismatched | ResponderMode::AudioThenMismatched => {
                    Some(ControlMessage::Ack {
                        session_id: session_id.saturating_add(1),
                        acked_kind,
                    })
                }
            };
            if let Some(response) = response {
                socket
                    .send_to(&encode_control(&response), source)
                    .expect("test responder must send an acknowledgement");
            }
        }
    });
    (address, handle)
}

fn run_test(mode: ResponderMode, session_id: u64) -> Result<(), SmokeError> {
    let (peer, responder) = spawn_responder(mode);
    let socket = UdpSocket::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, 0_u16)))
        .expect("smoke client must bind");
    let result = SmokeClient::new(&socket, peer, Duration::from_millis(20)).run(session_id);
    responder.join().expect("test responder must finish");
    result
}

#[test]
fn smoke_succeeds_with_three_matching_acknowledgements() {
    // Given
    // When
    let result = run_test(ResponderMode::Matching, 41);

    // Then
    assert!(result.is_ok());
}

#[test]
fn smoke_fails_when_acknowledgement_is_missing() {
    // Given
    // When
    let result = run_test(ResponderMode::Missing, 42);

    // Then
    assert!(matches!(
        result,
        Err(SmokeError::Transport(error))
            if matches!(
                error.kind(),
                io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock
            )
    ));
}

#[test]
fn smoke_fails_when_acknowledgement_session_mismatches() {
    // Given
    // When
    let result = run_test(ResponderMode::Mismatched, 43);

    // Then
    assert!(matches!(result, Err(SmokeError::MismatchedAck { .. })));
}

#[test]
fn smoke_succeeds_when_audio_precedes_matching_acknowledgement() {
    // Given
    // When
    let result = run_test(ResponderMode::AudioThenMatching, 44);

    // Then
    assert!(result.is_ok());
}

#[test]
fn smoke_times_out_when_only_audio_arrives_within_a_bounded_deadline() {
    // Given
    let started = Instant::now();

    // When
    let result = run_test(ResponderMode::AudioOnly, 45);

    // Then
    assert!(matches!(
        result,
        Err(SmokeError::Transport(error))
            if matches!(
                error.kind(),
                io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock
            )
    ));
    assert!(started.elapsed() < Duration::from_millis(250));
}

#[test]
fn smoke_fails_immediately_when_audio_precedes_mismatched_acknowledgement() {
    // Given
    // When
    let result = run_test(ResponderMode::AudioThenMismatched, 46);

    // Then
    assert!(matches!(result, Err(SmokeError::MismatchedAck { .. })));
}
