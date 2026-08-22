use std::fmt;
use std::time::{Duration, Instant};

use wifimic_diagnostics::{ErrorClass, Event};
use wifimic_protocol::{AudioFrame, ProtocolError};

use crate::capture::{CaptureError, CaptureHandle, CapturedFrame};

use super::{ControlPlane, CAPTURE_RETRY_INTERVAL};

/// The lifecycle of one client-controlled capture session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlState {
    Idle,
    Starting,
    Streaming,
}

/// The capture operations required by the control plane.
pub trait CaptureController {
    fn start(&mut self) -> Result<(), CaptureError>;
    fn stop(&mut self) -> Result<(), CaptureError>;
    fn read_frame(&mut self) -> Result<CapturedFrame, CaptureError>;
}

impl CaptureController for CaptureHandle {
    fn start(&mut self) -> Result<(), CaptureError> {
        CaptureHandle::start(self)
    }

    fn stop(&mut self) -> Result<(), CaptureError> {
        CaptureHandle::stop(self)
    }

    fn read_frame(&mut self) -> Result<CapturedFrame, CaptureError> {
        CaptureHandle::read_frame(self)
    }
}

/// A typed failure at the control-plane boundary.
#[derive(Debug)]
pub enum ControlError {
    Protocol(ProtocolError),
    UnexpectedAck { session_id: u64 },
    CaptureStop(CaptureError),
    CaptureRead(CaptureError),
}

impl fmt::Display for ControlError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Protocol(error) => write!(formatter, "invalid control datagram: {error}"),
            Self::UnexpectedAck { session_id } => {
                write!(
                    formatter,
                    "client sent an unexpected Ack for session {session_id}"
                )
            }
            Self::CaptureStop(error) => write!(formatter, "capture stop failed: {error}"),
            Self::CaptureRead(error) => write!(formatter, "capture read failed: {error}"),
        }
    }
}

impl std::error::Error for ControlError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Protocol(error) => Some(error),
            Self::CaptureStop(error) | Self::CaptureRead(error) => Some(error),
            Self::UnexpectedAck { .. } => None,
        }
    }
}

pub(crate) fn retry_deadline(now: Instant, interval: Duration) -> Instant {
    now.checked_add(interval).unwrap_or(now)
}

pub(crate) fn elapsed_millis(duration: Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}

pub(crate) fn capture_error_class(error: &CaptureError) -> ErrorClass {
    match error {
        CaptureError::Spawn { error, .. }
        | CaptureError::StdoutRead { error, .. }
        | CaptureError::Stop { error } => ErrorClass::from(error.kind()),
        CaptureError::StdoutUnavailable
        | CaptureError::StderrUnavailable
        | CaptureError::NotRunning
        | CaptureError::EndpointNotFound { .. }
        | CaptureError::StdoutClosed { .. } => ErrorClass::Other,
    }
}

impl<C> ControlPlane<C>
where
    C: CaptureController,
{
    /// Acquires one frame from the pinned source for the active session.
    pub fn next_audio_frame(
        &mut self,
        sequence: u32,
        now: Instant,
    ) -> Result<Option<AudioFrame>, ControlError> {
        let Some(session_id) = self.last_active_session_id else {
            return Ok(None);
        };
        if self.state != ControlState::Streaming {
            return Ok(None);
        }
        let captured = match self.capture.read_frame() {
            Ok(captured) => captured,
            Err(error) => {
                self.schedule_capture_retry(now, &error);
                return Ok(None);
            }
        };
        Ok(Some(AudioFrame::new(session_id, sequence, captured.pcm)))
    }

    pub(super) fn try_start(&mut self, now: Instant) {
        match self.capture.start() {
            Ok(()) => {
                self.state = ControlState::Streaming;
                self.next_retry_at = None;
                self.retry_attempt = 0;
            }
            Err(error) => self.schedule_capture_retry(now, &error),
        }
    }

    fn schedule_capture_retry(&mut self, now: Instant, error: &CaptureError) {
        self.state = ControlState::Starting;
        self.retry_attempt = self.retry_attempt.saturating_add(1);
        self.diagnostics.emit(
            now,
            Event::CaptureRetry {
                attempt: self.retry_attempt,
                error_kind: capture_error_class(error),
                retry_delay_ms: elapsed_millis(CAPTURE_RETRY_INTERVAL),
            },
        );
        self.next_retry_at = Some(retry_deadline(now, CAPTURE_RETRY_INTERVAL));
    }
}
