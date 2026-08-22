use std::io;
use std::net::SocketAddr;
use std::time::{Duration, Instant};

use thiserror::Error;
use wifimic_diagnostics::{Event, EventContext, SessionStopReason};
use wifimic_protocol::latency::{
    CalibrationResult, CalibrationTracker, CalibrationUpdate, RECALIBRATION_INTERVAL,
};
use wifimic_protocol::{
    encode_calibration, AudioFrame, CalibrationPacket, ControlMessage, SessionIdError,
    SessionIdGenerator, AUDIO_TAG, CALIBRATION_REPLY_TAG, PCM_PAYLOAD_BYTES,
};

use crate::jitter::{FrameInsertOutcome, JitterBuffer};
use crate::render::{RenderError, Renderer};

#[path = "control_support.rs"]
mod support;

pub use support::{
    DatagramTransport, LinuxPeerIp, ReceivedDatagram, UdpClientSocket, APPROVED_SERVER_IP,
};

/// The client-side control timers.
pub const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(5);
/// The wait before a new Start retry mints another session ID.
pub const RECONNECT_RETRY_INTERVAL: Duration = Duration::from_secs(5);
/// The maximum time allowed for a Start Ack.
pub const START_ACK_TIMEOUT: Duration = Duration::from_secs(5);
const MAX_MISSED_HEARTBEATS: u8 = 2;

/// The explicit client lifecycle state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClientState {
    /// A fresh Start is in flight and has not been acknowledged.
    Establishing,
    /// The current session's Start was acknowledged.
    Streaming,
    /// Two heartbeat Acks were missed; retry will establish a fresh session.
    Unreachable,
    /// Exit stopped this process run; no automatic work is scheduled.
    Stopped,
}

/// The safe result of processing one approved inbound datagram.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InboundOutcome {
    /// The source IP did not match the fixed Linux peer.
    DroppedUnapprovedSource,
    /// A Start Ack established this exact pending session.
    StartAck { session_id: u64 },
    /// A Heartbeat Ack confirmed or restored this exact session.
    HeartbeatAck { session_id: u64 },
    /// An Ack for a non-current session or stopped run was ignored.
    IgnoredAck { session_id: u64 },
    /// An approved audio frame entered the jitter buffer.
    AudioQueued {
        session_id: u64,
        outcome: FrameInsertOutcome,
    },
    /// An approved frame was decoded but is not for the accepted session.
    IgnoredAudio { session_id: u64 },
    /// A non-Ack control message from the peer was ignored.
    IgnoredControl,
    /// A calibration reply updated the active client clock offset.
    Calibrated {
        offset_us: i64,
        error_bound_us: u64,
        instability_warning: bool,
    },
    /// A calibration reply was rejected because its round trip was too long.
    CalibrationRejected,
}

/// The result of one renderer-facing playout poll.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderOutcome {
    /// The jitter target has not elapsed.
    NotReady,
    /// A decoded audio frame was rendered.
    Audio,
    /// A missing slot was rendered as silence.
    Gap,
}

/// Typed failures at the client control/audio boundaries.
#[derive(Debug, Error)]
pub enum ControlError {
    /// UDP transport failed.
    #[error(transparent)]
    Transport(#[from] io::Error),
    /// A received datagram was malformed.
    #[error(transparent)]
    Protocol(#[from] wifimic_protocol::ProtocolError),
    /// The in-process session ID generator exhausted its space.
    #[error(transparent)]
    Session(#[from] SessionIdError),
    /// Rendering the next playout slot failed.
    #[error(transparent)]
    Render(#[from] RenderError),
}

/// Injected process clocks and metadata sink for one client run.
#[derive(Debug, Clone)]
pub struct ControlConfig {
    origin: Instant,
    diagnostics: EventContext,
}

impl ControlConfig {
    /// Creates production configuration with the supplied monotonic origin.
    #[must_use]
    pub fn new(origin: Instant) -> Self {
        Self {
            origin,
            diagnostics: EventContext::logging(origin),
        }
    }

    /// Replaces the log sink with a deterministic metadata collector.
    #[must_use]
    pub fn with_diagnostics(mut self, diagnostics: EventContext) -> Self {
        self.diagnostics = diagnostics;
        self
    }
}

/// The render seam used by production WASAPI and deterministic tests.
pub trait AudioRenderer {
    /// Renders one mono protocol frame, or a silence frame for a jitter gap.
    fn render_frame(&mut self, frame: &AudioFrame) -> Result<(), RenderError>;
}

impl AudioRenderer for Renderer {
    fn render_frame(&mut self, frame: &AudioFrame) -> Result<(), RenderError> {
        Self::render_frame(self, frame)
    }
}

/// Owns one Windows client run's control, jitter, and render lifecycle.
pub struct ControlPlane<T, R> {
    transport: T,
    renderer: R,
    jitter: JitterBuffer,
    diagnostics: EventContext,
    origin: Instant,
    session_ids: SessionIdGenerator,
    state: ClientState,
    pending_session_id: Option<u64>,
    accepted_session_id: Option<u64>,
    start_deadline: Option<Instant>,
    next_heartbeat: Option<Instant>,
    next_retry: Option<Instant>,
    missed_heartbeats: u8,
    transition_count: u64,
    malformed_packets: u64,
    calibration: CalibrationTracker,
    next_calibration_at: Option<Instant>,
    calibration_sequence: u32,
    outstanding_calibration_sequence: Option<u32>,
}

impl<T, R> ControlPlane<T, R>
where
    T: DatagramTransport,
    R: AudioRenderer,
{
    /// Creates a client run with repository logging and an injected monotonic origin.
    #[must_use]
    pub fn new(transport: T, renderer: R, origin: Instant) -> Self {
        Self::with_config(transport, renderer, ControlConfig::new(origin))
    }

    /// Creates a client run with injected clocks and a metadata-only sink.
    #[must_use]
    pub fn with_config(transport: T, renderer: R, config: ControlConfig) -> Self {
        Self {
            transport,
            renderer,
            jitter: JitterBuffer::new(),
            diagnostics: config.diagnostics,
            origin: config.origin,
            session_ids: SessionIdGenerator::new(),
            state: ClientState::Establishing,
            pending_session_id: None,
            accepted_session_id: None,
            start_deadline: None,
            next_heartbeat: None,
            next_retry: None,
            missed_heartbeats: 0,
            transition_count: 0,
            malformed_packets: 0,
            calibration: CalibrationTracker::new(),
            next_calibration_at: None,
            calibration_sequence: 0,
            outstanding_calibration_sequence: None,
        }
    }

    /// Starts a new process run with a fresh session ID.
    pub fn start(&mut self, now: Instant, epoch_ms: u64) -> Result<u64, ControlError> {
        self.begin_establishing();
        self.issue_start(now, epoch_ms)
    }

    /// Replaces the current attempt with a fresh Start without sending Stop first.
    pub fn restart(&mut self, now: Instant, epoch_ms: u64) -> Result<u64, ControlError> {
        self.start(now, epoch_ms)
    }

    /// Sends Stop for the current attempt, then ends only this process run.
    pub fn stop(&mut self, now: Instant) -> Result<(), ControlError> {
        let session_id = self.pending_session_id.or(self.accepted_session_id);
        let result = session_id.map_or(Ok(()), |id| {
            self.send(ControlMessage::Stop { session_id: id })
        });
        self.state = ClientState::Stopped;
        self.pending_session_id = None;
        self.accepted_session_id = None;
        self.start_deadline = None;
        self.next_heartbeat = None;
        self.next_retry = None;
        self.outstanding_calibration_sequence = None;
        self.missed_heartbeats = 0;
        self.jitter.clear();
        if let Some(id) = session_id {
            self.diagnostics.clone().with_session_id(id).emit(
                now,
                Event::SessionStopped {
                    session_id: id,
                    reason: SessionStopReason::ExplicitRequest,
                },
            );
        }
        result
    }

    /// Advances timers without treating local UDP send success as reachability.
    pub fn advance(&mut self, now: Instant, epoch_ms: u64) -> Result<(), ControlError> {
        self.maybe_recalibrate(now)?;
        match self.state {
            ClientState::Stopped => Ok(()),
            ClientState::Establishing => {
                if self.start_deadline.is_some_and(|deadline| now >= deadline) {
                    self.mark_unreachable(now);
                    self.issue_start(now, epoch_ms).map(|_| ())
                } else {
                    Ok(())
                }
            }
            ClientState::Unreachable => {
                if self.next_retry.is_some_and(|retry| now >= retry) {
                    self.issue_start(now, epoch_ms).map(|_| ())
                } else {
                    Ok(())
                }
            }
            ClientState::Streaming => self.advance_streaming(now),
        }
    }

    /// Applies the source-IP boundary before decoding or mutating Ack/jitter state.
    pub fn receive_datagram(
        &mut self,
        source: SocketAddr,
        packet: &[u8],
        now: Instant,
    ) -> Result<InboundOutcome, ControlError> {
        if !LinuxPeerIp::configured().accepts(source) {
            return Ok(InboundOutcome::DroppedUnapprovedSource);
        }
        match packet.first().copied() {
            Some(AUDIO_TAG) => self.receive_audio(packet, now),
            Some(CALIBRATION_REPLY_TAG) => self.receive_calibration(packet, now),
            _ => self.receive_control(packet, now),
        }
    }

    /// Receives one transport datagram and applies the client boundary.
    pub fn receive_once(&mut self, now: Instant) -> Result<Option<InboundOutcome>, ControlError> {
        self.transport
            .receive_once()
            .map_err(ControlError::Transport)?
            .map_or(Ok(None), |datagram| {
                self.receive_datagram(datagram.source, &datagram.payload, now)
                    .map(Some)
            })
    }

    /// Polls jitter and renders one audio or silence slot.
    pub fn render_ready(&mut self, now: Instant) -> Result<RenderOutcome, ControlError> {
        let Some(item) = self.jitter.poll(self.elapsed_ms(now)).item() else {
            return Ok(RenderOutcome::NotReady);
        };
        let (frame, outcome) = match item.audio_frame() {
            Some(frame) => (frame, RenderOutcome::Audio),
            None => (
                AudioFrame::new(item.session_id(), item.sequence(), [0; PCM_PAYLOAD_BYTES]),
                RenderOutcome::Gap,
            ),
        };
        self.renderer.render_frame(&frame)?;
        Ok(outcome)
    }

    /// Returns the current lifecycle state.
    #[must_use]
    pub const fn state(&self) -> ClientState {
        self.state
    }

    /// Returns whether two heartbeat Acks have made this run unreachable.
    #[must_use]
    pub const fn is_unreachable(&self) -> bool {
        matches!(self.state, ClientState::Unreachable)
    }

    /// Returns the session whose Start Ack was accepted.
    #[must_use]
    pub const fn accepted_session_id(&self) -> Option<u64> {
        self.accepted_session_id
    }

    /// Borrows the transport for test and integration inspection.
    #[must_use]
    pub const fn transport(&self) -> &T {
        &self.transport
    }

    /// Borrows the renderer for test and integration inspection.
    #[must_use]
    pub const fn renderer(&self) -> &R {
        &self.renderer
    }

    /// Applies one accepted NTP-style sample and emits instability diagnostics.
    pub fn apply_calibration(
        &mut self,
        result: CalibrationResult,
        now: Instant,
    ) -> CalibrationUpdate {
        let previous_offset_us = self.calibration.offset_us();
        let update = self.calibration.update(result);
        if update.instability_warning {
            if let Some(previous_offset_us) = previous_offset_us {
                self.diagnostics.emit(
                    now,
                    Event::ClockInstabilityWarning {
                        previous_offset_us,
                        new_offset_us: update.offset_us,
                    },
                );
            }
        }
        update
    }

    fn maybe_recalibrate(&mut self, now: Instant) -> Result<(), ControlError> {
        if self.state != ClientState::Streaming
            || !self
                .next_calibration_at
                .is_some_and(|deadline| now >= deadline)
        {
            return Ok(());
        }
        let packet = encode_calibration(CalibrationPacket::Probe {
            sequence: self.calibration_sequence,
            t1_client_send_us: unix_micros(),
        });
        self.transport
            .send_to_peer(&packet)
            .map_err(ControlError::Transport)?;
        self.outstanding_calibration_sequence = Some(self.calibration_sequence);
        self.calibration_sequence = self.calibration_sequence.wrapping_add(1);
        self.next_calibration_at = Some(now + RECALIBRATION_INTERVAL);
        Ok(())
    }
}

fn unix_micros() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |duration| {
            u64::try_from(duration.as_micros()).unwrap_or(u64::MAX)
        })
}

#[path = "control_logic.rs"]
mod logic;

#[cfg(test)]
#[path = "control_tests.rs"]
mod tests;
