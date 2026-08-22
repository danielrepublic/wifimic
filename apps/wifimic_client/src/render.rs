use std::time::Duration;

use thiserror::Error;
use wifimic_protocol::{BYTES_PER_SAMPLE, PCM_PAYLOAD_BYTES};

/// The verified VB-CABLE playback endpoint. This is a configured target, not
/// a system-default fallback.
pub const DEFAULT_RENDER_ENDPOINT: &str = "CABLE Input (VB-Audio Virtual Cable)";
const DEFAULT_EVENT_WAIT: Duration = Duration::from_millis(100);
const MIN_EVENT_WAIT: Duration = Duration::from_millis(1);
pub(crate) const SAMPLES_PER_FRAME: usize = wifimic_protocol::SAMPLES_PER_FRAME;
pub(crate) const STEREO_CHANNELS: usize = 2;
pub(crate) const STEREO_FRAME_BYTES: usize = PCM_PAYLOAD_BYTES * STEREO_CHANNELS;

/// Errors produced by endpoint selection, stream timing, or the Windows backend.
#[derive(Debug, Error)]
pub enum RenderError {
    #[error("render endpoint name must not be empty")]
    InvalidEndpointName,
    #[error(
        "configured render endpoint '{expected}' was not found among {count} enumerated render endpoints: {available:?}",
        count = available.len()
    )]
    EndpointNotFound {
        expected: String,
        available: Vec<String>,
    },
    #[error("render event wait timeout after {wait_timeout_ms}ms")]
    EventWaitTimeout { wait_timeout_ms: u32 },
    #[error("render event wait timeout must be at least 1ms")]
    InvalidEventWaitTimeout,
    #[error(
        "render buffer has {available_frames} available frames, but {required_frames} are required"
    )]
    BufferTooSmall {
        available_frames: u32,
        required_frames: u32,
    },
    #[error("render buffer size overflow for {frames} frames of {bytes_per_frame} bytes")]
    BufferSizeOverflow { frames: u32, bytes_per_frame: u32 },
    #[error("WASAPI {operation} failed: {source}")]
    #[cfg(target_os = "windows")]
    Wasapi {
        operation: &'static str,
        #[source]
        source: wasapi::WasapiError,
    },
    #[error("COM MTA initialization failed with HRESULT 0x{hresult:08X}")]
    #[cfg(target_os = "windows")]
    ComInitialization { hresult: i32 },
    #[error("WASAPI rendering is only supported on Windows")]
    UnsupportedPlatform,
}

/// Chooses an endpoint by exact friendly-name match without default fallback.
pub(crate) fn select_endpoint_index(
    expected: &str,
    available: &[&str],
) -> Result<usize, RenderError> {
    if expected.trim().is_empty() {
        return Err(RenderError::InvalidEndpointName);
    }

    available
        .iter()
        .position(|name| *name == expected)
        .ok_or_else(|| RenderError::EndpointNotFound {
            expected: expected.to_owned(),
            available: available.iter().map(|name| (*name).to_owned()).collect(),
        })
}

/// Duplicates each little-endian mono sample into interleaved stereo bytes.
#[must_use]
pub(crate) fn mono_to_stereo_bytes(mono: &[u8; PCM_PAYLOAD_BYTES]) -> [u8; STEREO_FRAME_BYTES] {
    let mut stereo = [0_u8; STEREO_FRAME_BYTES];
    for (sample_index, sample) in mono.chunks_exact(BYTES_PER_SAMPLE).enumerate() {
        let stereo_start = sample_index * BYTES_PER_SAMPLE * STEREO_CHANNELS;
        stereo[stereo_start..stereo_start + BYTES_PER_SAMPLE].copy_from_slice(sample);
        stereo
            [stereo_start + BYTES_PER_SAMPLE..stereo_start + (BYTES_PER_SAMPLE * STEREO_CHANNELS)]
            .copy_from_slice(sample);
    }
    stereo
}

pub(crate) fn validate_buffer_capacity(
    available_frames: u32,
    required_frames: u32,
) -> Result<(), RenderError> {
    if available_frames < required_frames {
        return Err(RenderError::BufferTooSmall {
            available_frames,
            required_frames,
        });
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EventWaitOutcome {
    Signaled,
    TimedOut,
}

pub(crate) fn classify_event_wait(
    outcome: EventWaitOutcome,
    wait_timeout_ms: u32,
) -> Result<(), RenderError> {
    match outcome {
        EventWaitOutcome::Signaled => Ok(()),
        EventWaitOutcome::TimedOut => Err(RenderError::EventWaitTimeout { wait_timeout_ms }),
    }
}

/// Explicit render configuration consumed by later control/jitter code.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderConfig {
    endpoint_name: String,
    event_wait_timeout: Duration,
}

impl RenderConfig {
    /// Builds a configuration for one exact endpoint friendly name.
    pub fn new(endpoint_name: &str) -> Result<Self, RenderError> {
        if endpoint_name.trim().is_empty() {
            return Err(RenderError::InvalidEndpointName);
        }
        Ok(Self {
            endpoint_name: endpoint_name.to_owned(),
            event_wait_timeout: DEFAULT_EVENT_WAIT,
        })
    }

    /// Returns the verified VB-CABLE render configuration.
    #[must_use]
    pub fn vb_cable() -> Self {
        Self {
            endpoint_name: DEFAULT_RENDER_ENDPOINT.to_owned(),
            event_wait_timeout: DEFAULT_EVENT_WAIT,
        }
    }

    /// Sets the maximum time spent waiting for a render event.
    pub fn with_event_wait_timeout(mut self, timeout: Duration) -> Result<Self, RenderError> {
        if timeout < MIN_EVENT_WAIT {
            return Err(RenderError::InvalidEventWaitTimeout);
        }
        self.event_wait_timeout = timeout;
        Ok(self)
    }

    /// Returns the exact endpoint friendly name to select.
    #[must_use]
    pub fn endpoint_name(&self) -> &str {
        &self.endpoint_name
    }
}

impl Default for RenderConfig {
    fn default() -> Self {
        Self::vb_cable()
    }
}

#[cfg(target_os = "windows")]
#[path = "render_windows.rs"]
mod windows_backend;

#[cfg(target_os = "windows")]
pub use windows_backend::{enumerate_render_endpoints, Renderer};

#[cfg(not(target_os = "windows"))]
#[path = "render_non_windows.rs"]
mod non_windows_backend;

#[cfg(not(target_os = "windows"))]
pub use non_windows_backend::{enumerate_render_endpoints, Renderer};

#[cfg(test)]
#[path = "render_tests.rs"]
mod tests;
