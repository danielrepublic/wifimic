use std::time::{Duration, Instant};

use wasapi::{Device, DeviceEnumerator, Direction, SampleType, StreamMode, WaveFormat};
use wifimic_protocol::{AudioFrame, BITS_PER_SAMPLE, SAMPLE_RATE_HZ};

use super::{
    classify_event_wait, mono_to_stereo_bytes, select_endpoint_index, validate_buffer_capacity,
    EventWaitOutcome, RenderConfig, RenderError, SAMPLES_PER_FRAME, STEREO_CHANNELS,
};

/// A verified shared-mode, event-driven WASAPI render stream.
pub struct Renderer {
    _com: ComApartment,
    client: wasapi::AudioClient,
    render_client: wasapi::AudioRenderClient,
    event: wasapi::Handle,
    event_wait_timeout: Duration,
    started: bool,
}

impl Renderer {
    /// Enumerates active render endpoints, selects the exact configured
    /// endpoint, and starts a shared/event-driven stereo PCM stream.
    pub fn open(config: RenderConfig) -> Result<Self, RenderError> {
        let com = ComApartment::new()?;
        let enumerator = DeviceEnumerator::new().map_err(|source| RenderError::Wasapi {
            operation: "create render endpoint enumerator",
            source,
        })?;
        let device = select_device(&enumerator, config.endpoint_name())?;
        let mut client = device
            .get_iaudioclient()
            .map_err(|source| RenderError::Wasapi {
                operation: "activate verified render endpoint",
                source,
            })?;
        let format = WaveFormat::new(
            BITS_PER_SAMPLE as usize,
            BITS_PER_SAMPLE as usize,
            &SampleType::Int,
            SAMPLE_RATE_HZ as usize,
            STEREO_CHANNELS,
            None,
        );
        client
            .initialize_client(
                &format,
                &Direction::Render,
                &StreamMode::EventsShared {
                    autoconvert: true,
                    buffer_duration_hns: 0,
                },
            )
            .map_err(|source| RenderError::Wasapi {
                operation: "initialize shared event-driven render stream",
                source,
            })?;
        let event = client
            .set_get_eventhandle()
            .map_err(|source| RenderError::Wasapi {
                operation: "register render buffer event",
                source,
            })?;
        let render_client =
            client
                .get_audiorenderclient()
                .map_err(|source| RenderError::Wasapi {
                    operation: "obtain render buffer client",
                    source,
                })?;
        let buffer_frames = client
            .get_buffer_size()
            .map_err(|source| RenderError::Wasapi {
                operation: "query render buffer size",
                source,
            })?;
        validate_buffer_capacity(buffer_frames, SAMPLES_PER_FRAME as u32)?;
        let bytes_per_frame = format.get_blockalign();
        let silence_len = usize::try_from(buffer_frames)
            .ok()
            .and_then(|frames| {
                usize::try_from(bytes_per_frame)
                    .ok()
                    .and_then(|bytes| frames.checked_mul(bytes))
            })
            .ok_or(RenderError::BufferSizeOverflow {
                frames: buffer_frames,
                bytes_per_frame,
            })?;
        render_client
            .write_to_device(buffer_frames as usize, &vec![0_u8; silence_len], None)
            .map_err(|source| RenderError::Wasapi {
                operation: "prefill render buffer with silence",
                source,
            })?;
        client
            .start_stream()
            .map_err(|source| RenderError::Wasapi {
                operation: "start render stream",
                source,
            })?;

        Ok(Self {
            _com: com,
            client,
            render_client,
            event,
            event_wait_timeout: config.event_wait_timeout,
            started: true,
        })
    }

    /// Writes one protocol-owned mono PCM frame to the verified endpoint.
    pub fn render_frame(&self, frame: &AudioFrame) -> Result<(), RenderError> {
        let stereo = mono_to_stereo_bytes(&frame.pcm);
        self.wait_for_capacity(SAMPLES_PER_FRAME as u32)?;
        self.render_client
            .write_to_device(SAMPLES_PER_FRAME, &stereo, None)
            .map_err(|source| RenderError::Wasapi {
                operation: "write protocol PCM frame to render buffer",
                source,
            })
    }

    /// Stops the stream. Dropping the renderer also attempts this cleanup.
    pub fn stop(&mut self) -> Result<(), RenderError> {
        if !self.started {
            return Ok(());
        }
        self.client
            .stop_stream()
            .map_err(|source| RenderError::Wasapi {
                operation: "stop render stream",
                source,
            })?;
        self.started = false;
        Ok(())
    }

    fn wait_for_capacity(&self, required_frames: u32) -> Result<(), RenderError> {
        let started_at = Instant::now();
        loop {
            let elapsed = started_at.elapsed();
            if elapsed >= self.event_wait_timeout {
                return Err(RenderError::EventWaitTimeout {
                    wait_timeout_ms: duration_to_millis(self.event_wait_timeout),
                });
            }
            let remaining = self.event_wait_timeout - elapsed;
            let wait_timeout_ms = duration_to_millis(remaining).max(1);
            match self.event.wait_for_event(wait_timeout_ms) {
                Ok(()) => classify_event_wait(EventWaitOutcome::Signaled, wait_timeout_ms)?,
                Err(wasapi::WasapiError::EventTimeout) => {
                    classify_event_wait(EventWaitOutcome::TimedOut, wait_timeout_ms)?;
                }
                Err(source) => {
                    return Err(RenderError::Wasapi {
                        operation: "wait for render buffer event",
                        source,
                    });
                }
            }

            let available = self
                .client
                .get_available_space_in_frames()
                .map_err(|source| RenderError::Wasapi {
                    operation: "query available render buffer frames",
                    source,
                })?;
            if available >= required_frames {
                return validate_buffer_capacity(available, required_frames);
            }
        }
    }
}

impl Drop for Renderer {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

fn select_device(enumerator: &DeviceEnumerator, expected: &str) -> Result<Device, RenderError> {
    let collection = enumerator
        .get_device_collection(&Direction::Render)
        .map_err(|source| RenderError::Wasapi {
            operation: "enumerate active render endpoints",
            source,
        })?;
    let count = collection
        .get_nbr_devices()
        .map_err(|source| RenderError::Wasapi {
            operation: "count active render endpoints",
            source,
        })?;
    let mut names = Vec::new();
    let mut devices = Vec::new();
    for index in 0..count {
        let device =
            collection
                .get_device_at_index(index)
                .map_err(|source| RenderError::Wasapi {
                    operation: "read active render endpoint",
                    source,
                })?;
        names.push(
            device
                .get_friendlyname()
                .map_err(|source| RenderError::Wasapi {
                    operation: "read render endpoint friendly name",
                    source,
                })?,
        );
        devices.push(device);
    }
    let name_refs = names.iter().map(String::as_str).collect::<Vec<_>>();
    let selected = select_endpoint_index(expected, &name_refs)?;
    devices
        .into_iter()
        .nth(selected)
        .ok_or_else(|| RenderError::EndpointNotFound {
            expected: expected.to_owned(),
            available: names,
        })
}

fn duration_to_millis(duration: Duration) -> u32 {
    u32::try_from(duration.as_millis()).unwrap_or(u32::MAX)
}

/// Enumerates active render endpoint friendly names at startup.
pub fn enumerate_render_endpoints() -> Result<Vec<String>, RenderError> {
    let _com = ComApartment::new()?;
    let enumerator = DeviceEnumerator::new().map_err(|source| RenderError::Wasapi {
        operation: "create render endpoint enumerator",
        source,
    })?;
    let collection = enumerator
        .get_device_collection(&Direction::Render)
        .map_err(|source| RenderError::Wasapi {
            operation: "enumerate active render endpoints",
            source,
        })?;
    let count = collection
        .get_nbr_devices()
        .map_err(|source| RenderError::Wasapi {
            operation: "count active render endpoints",
            source,
        })?;
    (0..count)
        .map(|index| {
            collection
                .get_device_at_index(index)
                .map_err(|source| RenderError::Wasapi {
                    operation: "read active render endpoint",
                    source,
                })?
                .get_friendlyname()
                .map_err(|source| RenderError::Wasapi {
                    operation: "read render endpoint friendly name",
                    source,
                })
        })
        .collect()
}

struct ComApartment;

impl ComApartment {
    fn new() -> Result<Self, RenderError> {
        let result = wasapi::initialize_mta();
        if result.0 < 0 {
            return Err(RenderError::ComInitialization { hresult: result.0 });
        }
        Ok(Self)
    }
}

impl Drop for ComApartment {
    fn drop(&mut self) {
        wasapi::deinitialize();
    }
}
