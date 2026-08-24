use std::collections::VecDeque;

use wifimic_protocol::AudioFrame;

use super::{mono_to_stereo_bytes, RenderError, SAMPLES_PER_FRAME, STEREO_FRAME_BYTES};

const STEREO_DEVICE_FRAME_BYTES: usize = STEREO_FRAME_BYTES / SAMPLES_PER_FRAME;

/// A bounded stereo PCM FIFO whose enqueue operation never waits for WASAPI.
pub(super) struct PcmFifo {
    pcm: VecDeque<u8>,
    capacity_frames: usize,
    capacity_bytes: usize,
}

impl PcmFifo {
    pub(super) fn new(capacity_frames: usize) -> Self {
        Self {
            pcm: VecDeque::with_capacity(capacity_frames.saturating_mul(STEREO_FRAME_BYTES)),
            capacity_frames,
            capacity_bytes: capacity_frames.saturating_mul(STEREO_FRAME_BYTES),
        }
    }

    pub(super) fn push(&mut self, frame: &AudioFrame) -> Result<(), RenderError> {
        if self.capacity_frames == 0
            || self.pcm.len().saturating_add(STEREO_FRAME_BYTES) > self.capacity_bytes
        {
            return Err(RenderError::QueueFull {
                capacity_frames: self.capacity_frames,
            });
        }
        self.pcm.extend(mono_to_stereo_bytes(&frame.pcm));
        Ok(())
    }

    pub(super) fn queued_device_frames(&self) -> usize {
        self.pcm.len() / STEREO_DEVICE_FRAME_BYTES
    }

    pub(super) fn copy_front(&self, device_frames: usize, target: &mut Vec<u8>) {
        let byte_count = device_frames.saturating_mul(STEREO_DEVICE_FRAME_BYTES);
        target.clear();
        target.extend(self.pcm.iter().take(byte_count));
    }

    pub(super) fn discard_front(&mut self, device_frames: usize) {
        let byte_count = device_frames.saturating_mul(STEREO_DEVICE_FRAME_BYTES);
        drop(self.pcm.drain(..byte_count));
    }
}

pub(super) fn plan_render_frames(available_frames: u32, queued_device_frames: usize) -> usize {
    match usize::try_from(available_frames) {
        Ok(available) => queued_device_frames.min(available),
        Err(_) => 0,
    }
}
