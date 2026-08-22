use std::{io, time::Instant};

use super::{CaptureProcess, PcmFrame};
use wifimic_protocol::PCM_PAYLOAD_BYTES;

pub(crate) trait CaptureClock {
    fn now(&self) -> Instant;
}

pub(crate) struct SystemClock;

impl CaptureClock for SystemClock {
    fn now(&self) -> Instant {
        Instant::now()
    }
}

pub(super) enum FrameReadError {
    Eof { bytes_read: usize },
    Read { bytes_read: usize, error: io::Error },
}

pub(super) fn read_one_frame(process: &mut dyn CaptureProcess) -> Result<PcmFrame, FrameReadError> {
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

pub(super) fn endpoint_not_found(stderr: &str) -> bool {
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
