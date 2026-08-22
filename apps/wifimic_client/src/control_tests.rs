use std::io;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::time::{Duration, Instant};

use wifimic_protocol::{
    decode_control, encode_audio_frame, encode_control, AudioFrame, ControlMessage,
};

use super::{
    AudioRenderer, ClientState, ControlError, ControlPlane, DatagramTransport, InboundOutcome,
    RenderOutcome, APPROVED_SERVER_IP,
};

const SOURCE: SocketAddr = SocketAddr::new(IpAddr::V4(APPROVED_SERVER_IP), 52_000);
const OTHER_SOURCE: SocketAddr =
    SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 168, 0, 211)), 52_000);

#[derive(Debug, Default)]
struct FakeTransport {
    sent: Vec<Vec<u8>>,
}

impl DatagramTransport for FakeTransport {
    fn send_to_peer(&mut self, payload: &[u8]) -> io::Result<()> {
        self.sent.push(payload.to_vec());
        Ok(())
    }
}

#[derive(Debug, Default)]
struct FakeRenderer {
    frames: Vec<AudioFrame>,
}

impl AudioRenderer for FakeRenderer {
    fn render_frame(&mut self, frame: &AudioFrame) -> Result<(), super::RenderError> {
        self.frames.push(*frame);
        Ok(())
    }
}

fn client(origin: Instant) -> ControlPlane<FakeTransport, FakeRenderer> {
    ControlPlane::new(FakeTransport::default(), FakeRenderer::default(), origin)
}

fn last_message(client: &ControlPlane<FakeTransport, FakeRenderer>) -> ControlMessage {
    let packet = client
        .transport()
        .sent
        .last()
        .expect("a control packet must have been sent");
    decode_control(packet).expect("sent control bytes must decode")
}

fn ack(
    client: &mut ControlPlane<FakeTransport, FakeRenderer>,
    session_id: u64,
    acked_kind: u8,
    now: Instant,
) -> InboundOutcome {
    let packet = encode_control(&ControlMessage::Ack {
        session_id,
        acked_kind,
    });
    client
        .receive_datagram(SOURCE, &packet, now)
        .expect("matching Ack must be accepted")
}

#[test]
fn control_start_waits_for_ack_then_heartbeats_the_accepted_session() {
    // Given
    let origin = Instant::now();
    let mut client = client(origin);
    let session_id = client.start(origin, 10_000).expect("Start must send");
    assert_eq!(client.state(), ClientState::Establishing);

    // When
    let start_outcome = ack(&mut client, session_id, wifimic_protocol::START_TAG, origin);
    client
        .advance(origin + Duration::from_secs(5), 10_005)
        .expect("heartbeat tick must send");

    // Then
    assert!(
        matches!(start_outcome, InboundOutcome::StartAck { session_id: id } if id == session_id)
    );
    assert_eq!(client.state(), ClientState::Streaming);
    assert_eq!(
        last_message(&client),
        ControlMessage::Heartbeat { session_id }
    );
}

#[test]
fn control_auto_reconnects_after_two_missed_heartbeat_acks() {
    // Given
    let origin = Instant::now();
    let mut client = client(origin);
    let session_id = client.start(origin, 20_000).expect("Start must send");
    let _ = ack(&mut client, session_id, wifimic_protocol::START_TAG, origin);

    // When
    client
        .advance(origin + Duration::from_secs(5), 20_005)
        .expect("first heartbeat must send");
    client
        .advance(origin + Duration::from_secs(10), 20_010)
        .expect("second heartbeat must send");

    // Then
    assert!(client.is_unreachable());
    let _ = ack(
        &mut client,
        session_id,
        wifimic_protocol::HEARTBEAT_TAG,
        origin + Duration::from_secs(10),
    );
    assert_eq!(client.state(), ClientState::Streaming);
}

#[test]
fn control_drops_datagram_from_unapproved_source() {
    // Given
    let origin = Instant::now();
    let mut client = client(origin);
    let session_id = client.start(origin, 30_000).expect("Start must send");
    let packet = encode_control(&ControlMessage::Ack {
        session_id,
        acked_kind: wifimic_protocol::START_TAG,
    });

    // When
    let outcome = client
        .receive_datagram(OTHER_SOURCE, &packet, origin)
        .expect("unapproved source is a normal drop");

    // Then
    assert_eq!(outcome, InboundOutcome::DroppedUnapprovedSource);
    assert_eq!(client.state(), ClientState::Establishing);
}

#[test]
fn control_restart_ids_are_fresh_on_same_millisecond_and_backward_clock() {
    // Given
    let origin = Instant::now();
    let mut client = client(origin);
    let first = client.start(origin, 40_000).expect("Start must send");

    // When
    let second = client
        .restart(origin + Duration::from_millis(1), 40_000)
        .expect("Restart must send");
    let third = client
        .restart(origin + Duration::from_millis(2), 39_000)
        .expect("backward-clock Restart must send");

    // Then
    assert!(first < second && second < third);
    assert_eq!(
        last_message(&client),
        ControlMessage::Start { session_id: third }
    );
}

#[test]
fn control_reconnect_retries_use_fresh_session_ids_and_self_heal() {
    // Given
    let origin = Instant::now();
    let mut client = client(origin);
    let first = client.start(origin, 100).expect("Start must send");
    let mut issued = vec![first];

    // When
    for (seconds, epoch_ms) in [(5, 200), (10, 300), (15, 900), (20, 1_001)] {
        client
            .advance(origin + Duration::from_secs(seconds), epoch_ms)
            .expect("retry tick must send a fresh Start");
        let message = last_message(&client);
        let ControlMessage::Start { session_id } = message else {
            panic!("retry must send Start, got {message:?}");
        };
        issued.push(session_id);
    }
    let accepted = *issued.last().expect("four retries must issue IDs");
    let _ = ack(
        &mut client,
        accepted,
        wifimic_protocol::START_TAG,
        origin + Duration::from_secs(20),
    );

    // Then
    assert!(issued.windows(2).all(|pair| pair[0] < pair[1]));
    assert_eq!(client.accepted_session_id(), Some(1_001));
    assert_eq!(client.state(), ClientState::Streaming);
}

#[test]
fn control_stop_only_ends_current_run_and_explicit_restart_can_start_again() {
    // Given
    let origin = Instant::now();
    let mut client = client(origin);
    let first = client.start(origin, 50_000).expect("Start must send");
    let _ = ack(&mut client, first, wifimic_protocol::START_TAG, origin);
    let sent_before_stop = client.transport().sent.len();

    // When
    client
        .stop(origin + Duration::from_secs(1))
        .expect("Stop must send");
    let stop = last_message(&client);
    client
        .advance(origin + Duration::from_secs(10), 50_010)
        .expect("stopped client must remain quiet");

    // Then
    assert_eq!(stop, ControlMessage::Stop { session_id: first });
    assert_eq!(client.state(), ClientState::Stopped);
    assert_eq!(client.transport().sent.len(), sent_before_stop + 1);
    let second = client
        .restart(origin + Duration::from_secs(11), 50_011)
        .expect("a new explicit run must mint a new session");
    let _ = ack(
        &mut client,
        second,
        wifimic_protocol::START_TAG,
        origin + Duration::from_secs(11),
    );
    assert_eq!(client.state(), ClientState::Streaming);
}

#[test]
fn control_audio_uses_jitter_and_render_seams() {
    // Given
    let origin = Instant::now();
    let mut client = client(origin);
    let session_id = client.start(origin, 60_000).expect("Start must send");
    let _ = ack(&mut client, session_id, wifimic_protocol::START_TAG, origin);
    let frame = AudioFrame::new(session_id, 7, [3; wifimic_protocol::PCM_PAYLOAD_BYTES]);
    let packet = encode_audio_frame(&frame);

    // When
    let outcome = client
        .receive_datagram(SOURCE, &packet, origin)
        .expect("approved audio must decode");
    let early = client
        .render_ready(origin + Duration::from_millis(39))
        .expect("early render poll must succeed");
    let rendered = client
        .render_ready(origin + Duration::from_millis(40))
        .expect("render poll must succeed");

    // Then
    assert!(
        matches!(outcome, InboundOutcome::AudioQueued { session_id: id, .. } if id == session_id)
    );
    assert_eq!(early, RenderOutcome::NotReady);
    assert_eq!(rendered, RenderOutcome::Audio);
    assert_eq!(client.renderer().frames, vec![frame]);
}

#[test]
fn control_malformed_ack_is_typed_and_does_not_change_state() {
    // Given
    let origin = Instant::now();
    let mut client = client(origin);
    let _ = client.start(origin, 70_000).expect("Start must send");

    // When
    let result = client.receive_datagram(SOURCE, &[wifimic_protocol::ACK_TAG], origin);

    // Then
    assert!(matches!(result, Err(ControlError::Protocol(_))));
    assert_eq!(client.state(), ClientState::Establishing);
}
