use std::fmt;
use std::time::{Duration, Instant};

use wifimic_diagnostics::ErrorClass;
use wifimic_protocol::ProtocolError;

use crate::capture::{CaptureError, CaptureHandle};

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
}

impl CaptureController for CaptureHandle {
    fn start(&mut self) -> Result<(), CaptureError> {
        CaptureHandle::start(self)
    }

    fn stop(&mut self) -> Result<(), CaptureError> {
        CaptureHandle::stop(self)
    }
}

/// A typed failure at the control-plane boundary.
#[derive(Debug)]
pub enum ControlError {
    Protocol(ProtocolError),
    UnexpectedAck { session_id: u64 },
    CaptureStop(CaptureError),
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
        }
    }
}

impl std::error::Error for ControlError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Protocol(error) => Some(error),
            Self::CaptureStop(error) => Some(error),
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
