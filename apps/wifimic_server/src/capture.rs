use std::{path::PathBuf, time::Instant};

#[path = "capture_process.rs"]
mod process;
#[path = "capture_reader.rs"]
mod reader;
#[cfg(test)]
#[path = "capture_tests.rs"]
mod tests;
#[path = "capture_types.rs"]
mod types;

use process::ParecLauncher;
#[cfg(test)]
pub(super) use process::ProcessExit;
pub(super) use process::{CaptureLauncher, CaptureProcess};
use reader::{endpoint_not_found, read_one_frame, FrameReadError};
pub(super) use reader::{CaptureClock, SystemClock};
pub use types::{CaptureError, CapturedFrame, PcmFrame, PAREC_ARGUMENTS, PINNED_CAPTURE_SOURCE};
#[cfg(test)]
pub(super) use wifimic_protocol::PCM_PAYLOAD_BYTES;

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
        // `read_one_frame` returns only after the exact 480-byte frame has
        // been produced by parec. These timestamps therefore describe the
        // capture boundary, before ControlPlane or UDP processing begins.
        let outcome = read_one_frame(process.as_mut());

        match outcome {
            Ok(pcm) => {
                let observed_at = self.clock.now();
                let acquired_at_unix_us = self.clock.unix_micros();
                let acquired_at = self
                    .last_acquired_at
                    .map_or(observed_at, |previous| previous.max(observed_at));
                self.last_acquired_at = Some(acquired_at);
                Ok(CapturedFrame {
                    pcm,
                    acquired_at,
                    acquired_at_unix_us,
                })
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
