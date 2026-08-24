use std::time::Duration;

use wasapi::{DeviceEnumerator, Direction, SampleType, StreamMode, WaveFormat};
use wifimic_protocol::{BITS_PER_SAMPLE, SAMPLE_RATE_HZ};

use super::super::{RenderConfig, RenderError, STEREO_CHANNELS};
use super::endpoints::ComApartment;

const WASAPI_DEFAULT_BUFFER_DURATION_HNS: i64 = 0;

pub(super) struct RenderStream {
    _com: ComApartment,
    pub(super) client: wasapi::AudioClient,
    pub(super) render_client: wasapi::AudioRenderClient,
    pub(super) event: wasapi::Handle,
    pub(super) event_wait_timeout: Duration,
}

impl RenderStream {
    pub(super) fn open(config: &RenderConfig) -> Result<Self, RenderError> {
        let com = ComApartment::new()?;
        let enumerator = DeviceEnumerator::new().map_err(|source| RenderError::Wasapi {
            operation: "create render endpoint enumerator",
            source,
        })?;
        let device = super::endpoints::select_device(&enumerator, config.endpoint_name())?;
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
                    buffer_duration_hns: WASAPI_DEFAULT_BUFFER_DURATION_HNS,
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
            event_wait_timeout: config.event_wait_timeout(),
        })
    }

    pub(super) fn stop(&self) -> Result<(), RenderError> {
        self.client
            .stop_stream()
            .map_err(|source| RenderError::Wasapi {
                operation: "stop render stream",
                source,
            })
    }
}
