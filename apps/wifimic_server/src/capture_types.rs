use std::fmt;

use wifimic_protocol::PCM_PAYLOAD_BYTES;

/// The only capture endpoint accepted by the Linux server.
pub const PINNED_CAPTURE_SOURCE: &str = "alsa_input.pci-0000_00_1b.0.analog-stereo";

/// The exact `parec` arguments for the pinned PipeWire capture graph.
pub const PAREC_ARGUMENTS: [&str; 7] = [
    "--raw",
    "--format=s16le",
    "--rate=48000",
    "--channels=1",
    "--latency-msec=5",
    "--process-time-msec=5",
    "--device=alsa_input.pci-0000_00_1b.0.analog-stereo",
];

/// A fixed-size PCM frame produced by the pinned capture pipeline.
pub type PcmFrame = [u8; PCM_PAYLOAD_BYTES];

/// A PCM frame and the monotonic instant at which stdout produced it.
#[derive(Debug, Clone, Copy)]
pub struct CapturedFrame {
    /// Signed 16-bit little-endian mono PCM bytes.
    pub pcm: PcmFrame,
    /// Monotonic acquisition timestamp for this frame.
    pub acquired_at: std::time::Instant,
    /// Unix-microsecond acquisition timestamp for diagnostic correlation.
    pub acquired_at_unix_us: u64,
}

/// Failure from the pinned `parec` process or its stdout lifecycle.
#[derive(Debug)]
pub enum CaptureError {
    /// The capture process could not be started.
    Spawn {
        /// The source name that was requested.
        source_name: String,
        /// The operating-system error.
        error: std::io::Error,
    },
    /// The capture process did not provide stdout.
    StdoutUnavailable,
    /// The capture process did not provide stderr.
    StderrUnavailable,
    /// A frame was requested before capture was started.
    NotRunning,
    /// Reading stdout failed before a complete frame was produced.
    StdoutRead {
        /// Bytes already read into the incomplete frame.
        bytes_read: usize,
        /// The operating-system error.
        error: std::io::Error,
    },
    /// The source endpoint was absent and `parec` exited without substitution.
    EndpointNotFound {
        /// The missing pinned source name.
        source_name: String,
        /// Bounded diagnostic text from `parec`.
        stderr: String,
    },
    /// stdout closed before one complete frame was available.
    StdoutClosed {
        /// Bytes read into the incomplete frame.
        bytes_read: usize,
        /// The child exit code, if one was available.
        exit_code: Option<i32>,
        /// Bounded diagnostic text from `parec`.
        stderr: String,
    },
    /// The child could not be terminated and reaped.
    Stop {
        /// The operating-system error.
        error: std::io::Error,
    },
}

impl fmt::Display for CaptureError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Spawn { source_name, error } => {
                write!(formatter, "could not spawn parec for source {source_name}: {error}")
            }
            Self::StdoutUnavailable => formatter.write_str("parec stdout pipe was unavailable"),
            Self::StderrUnavailable => formatter.write_str("parec stderr pipe was unavailable"),
            Self::NotRunning => formatter.write_str("capture is not running"),
            Self::StdoutRead { bytes_read, error } => write!(
                formatter,
                "parec stdout read failed after {bytes_read} bytes: {error}"
            ),
            Self::EndpointNotFound {
                source_name,
                stderr,
            } => write!(
                formatter,
                "pinned capture source {source_name} was not found: {stderr}"
            ),
            Self::StdoutClosed {
                bytes_read,
                exit_code,
                stderr,
            } => write!(
                formatter,
                "parec stdout closed after {bytes_read} bytes with exit code {exit_code:?}: {stderr}"
            ),
            Self::Stop { error } => write!(formatter, "could not stop parec: {error}"),
        }
    }
}

impl std::error::Error for CaptureError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Spawn { error, .. } | Self::StdoutRead { error, .. } | Self::Stop { error } => {
                Some(error)
            }
            Self::StdoutUnavailable
            | Self::StderrUnavailable
            | Self::NotRunning
            | Self::EndpointNotFound { .. }
            | Self::StdoutClosed { .. } => None,
        }
    }
}
