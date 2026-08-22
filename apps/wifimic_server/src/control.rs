use std::time::{Duration, Instant};

use wifimic_diagnostics::{
    ControlMessageKind, ControlRejectionReason, Event, EventContext, SessionStopReason,
};
use wifimic_protocol::{
    decode_control, encode_control, ControlMessage, SessionOrder, HEARTBEAT_TAG, START_TAG,
    STOP_TAG,
};

#[path = "control_support.rs"]
mod support;

use support::{capture_error_class, elapsed_millis, retry_deadline};
pub use support::{CaptureController, ControlError, ControlState};

/// The only two timers owned by the server control plane.
pub const HEARTBEAT_TIMEOUT: Duration = Duration::from_secs(30);
pub const CAPTURE_RETRY_INTERVAL: Duration = Duration::from_secs(5);

/// Owns the Linux capture lifecycle and its session high-water mark.
pub struct ControlPlane<C> {
    capture: C,
    diagnostics: EventContext,
    session_order: SessionOrder,
    state: ControlState,
    last_active_session_id: Option<u64>,
    last_heartbeat_at: Option<Instant>,
    next_retry_at: Option<Instant>,
    retry_attempt: u32,
    malformed_packets: u64,
}

impl<C> ControlPlane<C>
where
    C: CaptureController,
{
    /// Creates an idle control plane with no persisted session state.
    #[must_use]
    pub fn new(capture: C, diagnostics: EventContext) -> Self {
        Self {
            capture,
            diagnostics,
            session_order: SessionOrder::new(),
            state: ControlState::Idle,
            last_active_session_id: None,
            last_heartbeat_at: None,
            next_retry_at: None,
            retry_attempt: 0,
            malformed_packets: 0,
        }
    }

    /// Returns the current explicit lifecycle state.
    #[must_use]
    pub const fn state(&self) -> ControlState {
        self.state
    }

    /// Returns the session high-water mark, including after Stop.
    #[must_use]
    pub const fn last_active_session_id(&self) -> Option<u64> {
        self.last_active_session_id
    }

    /// Decodes one control datagram and returns an encoded Ack when accepted.
    ///
    /// # Errors
    ///
    /// Returns a typed protocol or capture-stop error. Rejected commands are
    /// normal control traffic and return `Ok(None)` after emitting diagnostics.
    pub fn handle_datagram(
        &mut self,
        packet: &[u8],
        now: Instant,
    ) -> Result<Option<Vec<u8>>, ControlError> {
        self.advance(now)?;
        let message = match decode_control(packet) {
            Ok(message) => message,
            Err(error) => {
                self.malformed_packets = self.malformed_packets.saturating_add(1);
                self.diagnostics.emit(
                    now,
                    Event::MalformedPacket {
                        packet_len_bytes: packet.len(),
                        total_malformed_packets: self.malformed_packets,
                    },
                );
                return Err(ControlError::Protocol(error));
            }
        };
        self.handle_message_without_advance(message, now)
            .map(|ack| ack.map(|message| encode_control(&message)))
    }

    /// Runs the liveness timeout and one due capture retry.
    ///
    /// # Errors
    ///
    /// Returns a typed capture-stop error when the timeout cleanup fails.
    pub fn advance(&mut self, now: Instant) -> Result<(), ControlError> {
        if let Some(last_heartbeat_at) = self.last_heartbeat_at {
            let elapsed = now.saturating_duration_since(last_heartbeat_at);
            if elapsed >= HEARTBEAT_TIMEOUT {
                let session_id = self.last_active_session_id;
                self.diagnostics.emit(
                    now,
                    Event::HeartbeatTimeout {
                        elapsed_since_heartbeat_ms: elapsed_millis(elapsed),
                    },
                );
                if let Some(session_id) = session_id {
                    self.stop_and_idle(now, session_id, SessionStopReason::HeartbeatTimeout)?;
                }
                return Ok(());
            }
        }

        if self.state == ControlState::Starting
            && self.next_retry_at.is_some_and(|retry_at| now >= retry_at)
        {
            self.try_start(now);
        }
        Ok(())
    }

    fn handle_message_without_advance(
        &mut self,
        message: ControlMessage,
        now: Instant,
    ) -> Result<Option<ControlMessage>, ControlError> {
        match message {
            ControlMessage::Start { session_id } => self.handle_start(session_id, now),
            ControlMessage::Heartbeat { session_id } => self.handle_heartbeat(session_id, now),
            ControlMessage::Stop { session_id } => self.handle_stop(session_id, now),
            ControlMessage::Ack { session_id, .. } => {
                Err(ControlError::UnexpectedAck { session_id })
            }
        }
    }

    fn handle_start(
        &mut self,
        session_id: u64,
        now: Instant,
    ) -> Result<Option<ControlMessage>, ControlError> {
        if !self.session_order.accept(session_id) {
            self.reject(
                now,
                session_id,
                ControlMessageKind::Start,
                ControlRejectionReason::StaleSession,
            );
            return Ok(None);
        }

        self.last_active_session_id = Some(session_id);
        self.last_heartbeat_at = Some(now);
        self.diagnostics
            .clone()
            .with_session_id(session_id)
            .emit(now, Event::SessionStarted { session_id });

        match self.state {
            ControlState::Idle => {
                self.state = ControlState::Starting;
                self.retry_attempt = 0;
                self.try_start(now);
            }
            ControlState::Starting => {
                self.retry_attempt = 0;
                self.next_retry_at = Some(retry_deadline(now, CAPTURE_RETRY_INTERVAL));
            }
            ControlState::Streaming => {}
        }

        Ok(Some(ControlMessage::Ack {
            session_id,
            acked_kind: START_TAG,
        }))
    }

    fn handle_heartbeat(
        &mut self,
        session_id: u64,
        now: Instant,
    ) -> Result<Option<ControlMessage>, ControlError> {
        if self.last_active_session_id != Some(session_id) {
            self.reject(
                now,
                session_id,
                ControlMessageKind::Heartbeat,
                ControlRejectionReason::SessionMismatch,
            );
            return Ok(None);
        }
        if self.state != ControlState::Streaming {
            self.reject(
                now,
                session_id,
                ControlMessageKind::Heartbeat,
                ControlRejectionReason::InactiveSession,
            );
            return Ok(None);
        }

        self.last_heartbeat_at = Some(now);
        Ok(Some(ControlMessage::Ack {
            session_id,
            acked_kind: HEARTBEAT_TAG,
        }))
    }

    fn handle_stop(
        &mut self,
        session_id: u64,
        now: Instant,
    ) -> Result<Option<ControlMessage>, ControlError> {
        if self.last_active_session_id != Some(session_id) {
            self.reject(
                now,
                session_id,
                ControlMessageKind::Stop,
                ControlRejectionReason::SessionMismatch,
            );
            return Ok(None);
        }

        if self.state != ControlState::Idle {
            self.stop_and_idle(now, session_id, SessionStopReason::ExplicitRequest)?;
        }
        Ok(Some(ControlMessage::Ack {
            session_id,
            acked_kind: STOP_TAG,
        }))
    }

    fn try_start(&mut self, now: Instant) {
        match self.capture.start() {
            Ok(()) => {
                self.state = ControlState::Streaming;
                self.next_retry_at = None;
                self.retry_attempt = 0;
            }
            Err(error) => {
                self.retry_attempt = self.retry_attempt.saturating_add(1);
                self.diagnostics.emit(
                    now,
                    Event::CaptureRetry {
                        attempt: self.retry_attempt,
                        error_kind: capture_error_class(&error),
                        retry_delay_ms: elapsed_millis(CAPTURE_RETRY_INTERVAL),
                    },
                );
                self.next_retry_at = Some(retry_deadline(now, CAPTURE_RETRY_INTERVAL));
            }
        }
    }

    fn stop_and_idle(
        &mut self,
        now: Instant,
        session_id: u64,
        reason: SessionStopReason,
    ) -> Result<(), ControlError> {
        self.state = ControlState::Idle;
        self.last_heartbeat_at = None;
        self.next_retry_at = None;
        self.retry_attempt = 0;
        self.capture.stop().map_err(ControlError::CaptureStop)?;
        self.diagnostics
            .clone()
            .with_session_id(session_id)
            .emit(now, Event::SessionStopped { session_id, reason });
        Ok(())
    }

    fn reject(
        &self,
        now: Instant,
        session_id: u64,
        kind: ControlMessageKind,
        reason: ControlRejectionReason,
    ) {
        self.diagnostics
            .clone()
            .with_session_id(session_id)
            .emit(now, Event::ControlMessageRejected { kind, reason });
    }
}

#[cfg(test)]
#[path = "control_tests.rs"]
mod tests;
