use std::{fmt, io, path::PathBuf, time::Instant};

use wifimic_protocol::PCM_PAYLOAD_BYTES;

#[path = "capture_process.rs"]
mod process;
#[cfg(test)]
#[path = "capture_tests.rs"]
mod tests;

use process::ParecLauncher;

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
    pub acquired_at: Instant,
}

/// Failure from the pinned `parec` process or its stdout lifecycle.
#[derive(Debug)]
pub enum CaptureError {
    /// The capture process could not be started.
    Spawn {
        /// The source name that was requested.
        source_name: String,
        /// The operating-system error.
        error: io::Error,
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
        error: io::Error,
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
        error: io::Error,
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

/// Owns a lazily-created `parec` process and produces exact protocol frames.
pub struct CaptureHandle {
    launcher: Box<dyn CaptureLauncher>,
    process: Option<Box<dyn CaptureProcess>>,
    clock: Box<dyn CaptureClock>,
    last_acquired_at: Option<Instant>,
}

impl CaptureHandle {
    /// Constructs an idle handle for the fixed `parec` source.
    #[must_use]
    pub fn new() -> Self {
        Self::with_components(
            Box::new(ParecLauncher::new(PathBuf::from("parec"))),
            Box::new(SystemClock),
        )
    }

    /// Starts the pinned capture process, if it is not already running.
    ///
    /// # Errors
    ///
    /// Returns [`CaptureError::Spawn`] when `parec` cannot be launched.
    pub fn start(&mut self) -> Result<(), CaptureError> {
        if self.process.is_none() {
            self.process = Some(self.launcher.spawn(&PAREC_ARGUMENTS)?);
        }
        Ok(())
    }

    /// Returns whether a capture process is currently owned by this handle.
    #[must_use]
    pub fn is_running(&self) -> bool {
        self.process.is_some()
    }

    /// Reads one exact PCM frame from stdout and timestamps its production.
    ///
    /// # Errors
    ///
    /// Returns a typed error when capture is idle, stdout is malformed, or the
    /// pinned endpoint is absent.
    pub fn read_frame(&mut self) -> Result<CapturedFrame, CaptureError> {
        let Some(process) = self.process.as_mut() else {
            return Err(CaptureError::NotRunning);
        };
        let outcome = read_one_frame(process.as_mut());

        match outcome {
            Ok(pcm) => {
                let observed_at = self.clock.now();
                let acquired_at = self
                    .last_acquired_at
                    .map_or(observed_at, |previous| previous.max(observed_at));
                self.last_acquired_at = Some(acquired_at);
                Ok(CapturedFrame { pcm, acquired_at })
            }
            Err(FrameReadError::Eof { bytes_read }) => {
                let Some(mut process) = self.process.take() else {
                    return Err(CaptureError::NotRunning);
                };
                let exit = process
                    .finish_after_eof()
                    .map_err(|error| CaptureError::StdoutRead { bytes_read, error })?;
                if !exit.success && endpoint_not_found(&exit.stderr) {
                    return Err(CaptureError::EndpointNotFound {
                        source_name: PINNED_CAPTURE_SOURCE.to_owned(),
                        stderr: exit.stderr,
                    });
                }
                Err(CaptureError::StdoutClosed {
                    bytes_read,
                    exit_code: exit.exit_code,
                    stderr: exit.stderr,
                })
            }
            Err(FrameReadError::Read { bytes_read, error }) => {
                Err(CaptureError::StdoutRead { bytes_read, error })
            }
        }
    }

    /// Stops and reaps the capture process; repeated calls are harmless.
    ///
    /// # Errors
    ///
    /// Returns [`CaptureError::Stop`] when the child cannot be terminated or
    /// reaped.
    pub fn stop(&mut self) -> Result<(), CaptureError> {
        let Some(mut process) = self.process.take() else {
            return Ok(());
        };
        process.stop().map_err(|error| CaptureError::Stop { error })
    }

    fn with_components(launcher: Box<dyn CaptureLauncher>, clock: Box<dyn CaptureClock>) -> Self {
        Self {
            launcher,
            process: None,
            clock,
            last_acquired_at: None,
        }
    }

    #[cfg(test)]
    fn with_test_components(
        launcher: Box<dyn CaptureLauncher>,
        clock: Box<dyn CaptureClock>,
    ) -> Self {
        Self::with_components(launcher, clock)
    }
}

impl Default for CaptureHandle {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for CaptureHandle {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

trait CaptureLauncher {
    fn spawn(&self, arguments: &[&str]) -> Result<Box<dyn CaptureProcess>, CaptureError>;
}

trait CaptureProcess {
    fn read_stdout(&mut self, buffer: &mut [u8]) -> io::Result<usize>;
    fn finish_after_eof(&mut self) -> io::Result<ProcessExit>;
    fn stop(&mut self) -> io::Result<()>;
}

trait CaptureClock {
    fn now(&self) -> Instant;
}

struct SystemClock;

impl CaptureClock for SystemClock {
    fn now(&self) -> Instant {
        Instant::now()
    }
}

#[derive(Debug, Clone)]
struct ProcessExit {
    success: bool,
    exit_code: Option<i32>,
    stderr: String,
}

enum FrameReadError {
    Eof { bytes_read: usize },
    Read { bytes_read: usize, error: io::Error },
}

fn read_one_frame(process: &mut dyn CaptureProcess) -> Result<PcmFrame, FrameReadError> {
    let mut frame = [0_u8; PCM_PAYLOAD_BYTES];
    let mut bytes_read = 0_usize;

    while bytes_read < frame.len() {
        match process.read_stdout(&mut frame[bytes_read..]) {
            Ok(0) => return Err(FrameReadError::Eof { bytes_read }),
            Ok(read) => bytes_read += read,
            Err(error) => return Err(FrameReadError::Read { bytes_read, error }),
        }
    }

    Ok(frame)
}

fn endpoint_not_found(stderr: &str) -> bool {
    let normalized = stderr.to_ascii_lowercase();
    [
        "no such entity",
        "source not found",
        "unknown source",
        "source does not exist",
    ]
    .iter()
    .any(|marker| normalized.contains(marker))
}
