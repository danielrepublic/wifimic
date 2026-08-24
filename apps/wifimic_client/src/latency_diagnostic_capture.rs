use thiserror::Error;
use wasapi::{Direction, SampleType, StreamMode, WaveFormat};

use super::CAPTURE_ENDPOINT;

#[path = "latency_diagnostic_capture_endpoints.rs"]
mod endpoints;

const WASAPI_DEFAULT_BUFFER_DURATION_HNS: i64 = 0;
const CAPTURE_CHANNELS: usize = 2;
const CAPTURE_WAIT_TIMEOUT_MS: u32 = 2;

#[derive(Debug, Error)]
pub(crate) enum CaptureError {
    #[error("capture endpoint name must not be empty")]
    InvalidEndpointName,
    #[error(
        "configured capture endpoint '{expected}' was not found among {count} enumerated capture endpoints: {available:?}",
        count = available.len()
    )]
    EndpointNotFound {
        expected: String,
        available: Vec<String>,
    },
    #[error("capture buffer size overflow for {frames} frames of {bytes_per_frame} bytes")]
    BufferSizeOverflow { frames: u32, bytes_per_frame: u32 },
    #[error("WASAPI {operation} failed: {source}")]
    Wasapi {
        operation: &'static str,
        #[source]
        source: wasapi::WasapiError,
    },
    #[error("COM MTA initialization failed with HRESULT 0x{hresult:08X}")]
    ComInitialization { hresult: i32 },
}

pub(super) struct CapturedPacket {
    pub(super) acquired_at_us: u64,
    pub(super) samples: Vec<i16>,
}

struct ComApartment;

impl ComApartment {
    fn new() -> Result<Self, CaptureError> {
        let result = wasapi::initialize_mta();
        if result.0 < 0 {
            return Err(CaptureError::ComInitialization { hresult: result.0 });
        }
        Ok(Self)
    }
}

impl Drop for ComApartment {
    fn drop(&mut self) {
        wasapi::deinitialize();
    }
}

pub(super) struct CaptureStream {
    _com: ComApartment,
    client: wasapi::AudioClient,
    capture_client: wasapi::AudioCaptureClient,
    event: wasapi::Handle,
    bytes_per_frame: u32,
    started: bool,
}

impl CaptureStream {
    pub(super) fn open() -> Result<Self, CaptureError> {
        let com = ComApartment::new()?;
        let enumerator =
            wasapi::DeviceEnumerator::new().map_err(|source| CaptureError::Wasapi {
                operation: "create capture endpoint enumerator",
                source,
            })?;
        let device = endpoints::select_device(&enumerator, CAPTURE_ENDPOINT)?;
        let mut client = device
            .get_iaudioclient()
            .map_err(|source| CaptureError::Wasapi {
                operation: "activate verified capture endpoint",
                source,
            })?;
        let format = WaveFormat::new(
            wifimic_protocol::BITS_PER_SAMPLE as usize,
            wifimic_protocol::BITS_PER_SAMPLE as usize,
            &SampleType::Int,
            wifimic_protocol::SAMPLE_RATE_HZ as usize,
            CAPTURE_CHANNELS,
            None,
        );
        client
            .initialize_client(
                &format,
                &Direction::Capture,
                &StreamMode::EventsShared {
                    autoconvert: true,
                    buffer_duration_hns: WASAPI_DEFAULT_BUFFER_DURATION_HNS,
                },
            )
            .map_err(|source| CaptureError::Wasapi {
                operation: "initialize shared event-driven capture stream",
                source,
            })?;
        let event = client
            .set_get_eventhandle()
            .map_err(|source| CaptureError::Wasapi {
                operation: "register capture buffer event",
                source,
            })?;
        let capture_client =
            client
                .get_audiocaptureclient()
                .map_err(|source| CaptureError::Wasapi {
                    operation: "obtain capture buffer client",
                    source,
                })?;
        client
            .start_stream()
            .map_err(|source| CaptureError::Wasapi {
                operation: "start capture stream",
                source,
            })?;
        Ok(Self {
            _com: com,
            client,
            capture_client,
            event,
            bytes_per_frame: format.get_blockalign(),
            started: true,
        })
    }

    pub(super) fn discard_available(&self) -> Result<(), CaptureError> {
        let _ = self.read_available()?;
        Ok(())
    }

    pub(super) fn read_available(&self) -> Result<Vec<CapturedPacket>, CaptureError> {
        match self.event.wait_for_event(CAPTURE_WAIT_TIMEOUT_MS) {
            Ok(()) | Err(wasapi::WasapiError::EventTimeout) => {}
            Err(source) => {
                return Err(CaptureError::Wasapi {
                    operation: "wait for capture buffer event",
                    source,
                });
            }
        }
        let mut packets = Vec::new();
        while let Some(packet_frames) =
            self.capture_client
                .get_next_packet_size()
                .map_err(|source| CaptureError::Wasapi {
                    operation: "query capture packet size",
                    source,
                })?
        {
            if packet_frames == 0 {
                break;
            }
            let byte_count = usize::try_from(packet_frames)
                .ok()
                .and_then(|frames| {
                    usize::try_from(self.bytes_per_frame)
                        .ok()
                        .and_then(|bytes| frames.checked_mul(bytes))
                })
                .ok_or(CaptureError::BufferSizeOverflow {
                    frames: packet_frames,
                    bytes_per_frame: self.bytes_per_frame,
                })?;
            let mut bytes = vec![0_u8; byte_count];
            let (frames, _info) =
                self.capture_client
                    .read_from_device(&mut bytes)
                    .map_err(|source| CaptureError::Wasapi {
                        operation: "read capture packet",
                        source,
                    })?;
            if frames == 0 {
                continue;
            }
            let valid_bytes = usize::try_from(frames)
                .ok()
                .and_then(|count| {
                    usize::try_from(self.bytes_per_frame)
                        .ok()
                        .and_then(|bytes_per_frame| count.checked_mul(bytes_per_frame))
                })
                .ok_or(CaptureError::BufferSizeOverflow {
                    frames,
                    bytes_per_frame: self.bytes_per_frame,
                })?;
            let samples = bytes[..valid_bytes]
                .chunks_exact(wifimic_protocol::BYTES_PER_SAMPLE * CAPTURE_CHANNELS)
                .map(|frame| i16::from_le_bytes([frame[0], frame[1]]))
                .collect();
            packets.push(CapturedPacket {
                acquired_at_us: unix_micros(),
                samples,
            });
        }
        Ok(packets)
    }

    fn stop(&mut self) -> Result<(), CaptureError> {
        if !self.started {
            return Ok(());
        }
        self.client
            .stop_stream()
            .map_err(|source| CaptureError::Wasapi {
                operation: "stop capture stream",
                source,
            })?;
        self.started = false;
        Ok(())
    }
}

impl Drop for CaptureStream {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

fn unix_micros() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |duration| {
            u64::try_from(duration.as_micros()).unwrap_or(u64::MAX)
        })
}

#[cfg(test)]
#[path = "latency_diagnostic_capture_tests.rs"]
mod tests;
