use std::time::Instant;

use wifimic_diagnostics::{ConnectionState, Event};
use wifimic_protocol::{
    decode_audio_frame, decode_control, encode_control, ControlMessage, HEARTBEAT_TAG, START_TAG,
};

use super::{
    ClientState, ControlError, ControlPlane, InboundOutcome, HEARTBEAT_INTERVAL,
    MAX_MISSED_HEARTBEATS, RECONNECT_RETRY_INTERVAL, START_ACK_TIMEOUT,
};

impl<T, R> ControlPlane<T, R>
where
    T: super::DatagramTransport,
    R: super::AudioRenderer,
{
    pub(super) fn receive_control(
        &mut self,
        packet: &[u8],
        now: Instant,
    ) -> Result<InboundOutcome, ControlError> {
        let message = decode_control(packet).inspect_err(|_error| {
            self.malformed_packets = self.malformed_packets.saturating_add(1);
            self.diagnostics.emit(
                now,
                Event::MalformedPacket {
                    packet_len_bytes: packet.len(),
                    total_malformed_packets: self.malformed_packets,
                },
            );
        })?;
        match message {
            ControlMessage::Ack {
                session_id,
                acked_kind: START_TAG,
            } if self.pending_session_id == Some(session_id) => {
                self.accept_start(session_id, now);
                Ok(InboundOutcome::StartAck { session_id })
            }
            ControlMessage::Ack {
                session_id,
                acked_kind: HEARTBEAT_TAG,
            } if self.accepted_session_id == Some(session_id)
                && matches!(
                    self.state,
                    ClientState::Streaming | ClientState::Unreachable
                ) =>
            {
                self.accept_heartbeat(now);
                Ok(InboundOutcome::HeartbeatAck { session_id })
            }
            ControlMessage::Ack { session_id, .. } => Ok(InboundOutcome::IgnoredAck { session_id }),
            ControlMessage::Start { .. }
            | ControlMessage::Heartbeat { .. }
            | ControlMessage::Stop { .. } => Ok(InboundOutcome::IgnoredControl),
        }
    }

    pub(super) fn receive_audio(
        &mut self,
        packet: &[u8],
        now: Instant,
    ) -> Result<InboundOutcome, ControlError> {
        let frame = decode_audio_frame(packet)?;
        if self.state != ClientState::Streaming
            || self.accepted_session_id != Some(frame.session_id)
        {
            return Ok(InboundOutcome::IgnoredAudio {
                session_id: frame.session_id,
            });
        }
        let outcome = self.jitter.push(frame, self.elapsed_ms(now));
        Ok(InboundOutcome::AudioQueued {
            session_id: frame.session_id,
            outcome,
        })
    }

    pub(super) fn advance_streaming(&mut self, now: Instant) -> Result<(), ControlError> {
        if !self.next_heartbeat.is_some_and(|next| now >= next) {
            return Ok(());
        }
        let Some(session_id) = self.accepted_session_id else {
            return Ok(());
        };
        let missed = self.missed_heartbeats.saturating_add(1);
        self.send(ControlMessage::Heartbeat { session_id })?;
        self.missed_heartbeats = missed;
        self.next_heartbeat = Some(now + HEARTBEAT_INTERVAL);
        if missed >= MAX_MISSED_HEARTBEATS {
            self.mark_unreachable(now);
        }
        Ok(())
    }

    pub(super) fn accept_start(&mut self, session_id: u64, now: Instant) {
        let reconnect = self.state == ClientState::Unreachable;
        self.pending_session_id = None;
        self.accepted_session_id = Some(session_id);
        self.start_deadline = None;
        self.next_retry = None;
        self.next_heartbeat = Some(now + HEARTBEAT_INTERVAL);
        self.missed_heartbeats = 0;
        self.jitter.clear();
        self.state = ClientState::Streaming;
        self.transition_count = self.transition_count.saturating_add(1);
        self.diagnostics
            .clone()
            .with_session_id(session_id)
            .emit(now, Event::SessionStarted { session_id });
        self.diagnostics.emit(
            now,
            Event::ConnectionTransition {
                state: ConnectionState::Streaming,
                reconnected: reconnect,
                total_transitions: self.transition_count,
            },
        );
    }

    pub(super) fn accept_heartbeat(&mut self, now: Instant) {
        let reconnected = self.state == ClientState::Unreachable;
        self.state = ClientState::Streaming;
        self.next_retry = None;
        self.next_heartbeat = Some(now + HEARTBEAT_INTERVAL);
        self.missed_heartbeats = 0;
        if reconnected {
            self.transition_count = self.transition_count.saturating_add(1);
            self.diagnostics.emit(
                now,
                Event::ConnectionTransition {
                    state: ConnectionState::Streaming,
                    reconnected: true,
                    total_transitions: self.transition_count,
                },
            );
        }
    }

    pub(super) fn begin_establishing(&mut self) {
        self.state = ClientState::Establishing;
        self.pending_session_id = None;
        self.accepted_session_id = None;
        self.start_deadline = None;
        self.next_heartbeat = None;
        self.next_retry = None;
        self.missed_heartbeats = 0;
        self.jitter.clear();
    }

    pub(super) fn issue_start(&mut self, now: Instant, epoch_ms: u64) -> Result<u64, ControlError> {
        let session_id = self.session_ids.next_id(epoch_ms)?;
        self.accepted_session_id = None;
        self.pending_session_id = Some(session_id);
        self.start_deadline = Some(now + START_ACK_TIMEOUT);
        self.next_retry = None;
        self.state = ClientState::Establishing;
        self.send(ControlMessage::Start { session_id })?;
        Ok(session_id)
    }

    pub(super) fn mark_unreachable(&mut self, now: Instant) {
        if self.state == ClientState::Unreachable {
            return;
        }
        self.state = ClientState::Unreachable;
        self.next_retry = Some(now + RECONNECT_RETRY_INTERVAL);
        self.jitter.clear();
        self.transition_count = self.transition_count.saturating_add(1);
        self.diagnostics.emit(
            now,
            Event::ConnectionTransition {
                state: ConnectionState::Disconnected,
                reconnected: false,
                total_transitions: self.transition_count,
            },
        );
    }

    pub(super) fn send(&mut self, message: ControlMessage) -> Result<(), ControlError> {
        self.transport
            .send_to_peer(&encode_control(&message))
            .map_err(ControlError::Transport)
    }

    pub(super) fn elapsed_ms(&self, now: Instant) -> u64 {
        u64::try_from(now.saturating_duration_since(self.origin).as_millis()).unwrap_or(u64::MAX)
    }
}
