use wifimic_protocol::AudioFrame;

use super::buffer::{BufferedFrame, JitterBuffer};
use super::{
    FRAME_DURATION_MS, GROWTH_THRESHOLD_FRAMES, HALF_SEQUENCE_RANGE, LATE_TOLERANCE_MS,
    MAX_BUFFERED_FRAMES, MAX_TARGET_DELAY_MS, MIN_TARGET_DELAY_MS, STABLE_PERIOD_FRAMES,
    TARGET_DECAY_STEP_MS, TARGET_GROWTH_STEP_MS,
};

impl JitterBuffer {
    pub(super) fn is_duplicate(&self, sequence: u32) -> bool {
        self.last_played == Some(sequence)
            || self
                .frames
                .iter()
                .any(|buffered| buffered.frame.sequence == sequence)
    }

    pub(super) fn is_future(&self, sequence: u32) -> bool {
        self.next_sequence.is_some_and(|next| {
            let distance = sequence.wrapping_sub(next);
            distance > 0 && distance < HALF_SEQUENCE_RANGE
        })
    }

    pub(super) fn is_outside_window(&self, sequence: u32) -> bool {
        let Ok(max_buffered_frames) = u32::try_from(MAX_BUFFERED_FRAMES) else {
            return false;
        };
        self.next_sequence.is_some_and(|next| {
            let distance = sequence.wrapping_sub(next);
            distance >= max_buffered_frames && distance < HALF_SEQUENCE_RANGE
        })
    }

    pub(super) fn queue_frame(&mut self, frame: AudioFrame) {
        let Some(next_sequence) = self.next_sequence else {
            self.next_sequence = Some(frame.sequence);
            self.frames.push_back(BufferedFrame { frame });
            return;
        };
        let distance = frame.sequence.wrapping_sub(next_sequence);
        if self.frames.len() >= MAX_BUFFERED_FRAMES {
            let _ = self.frames.pop_front();
        }
        let position = self
            .frames
            .iter()
            .position(|buffered| buffered.frame.sequence.wrapping_sub(next_sequence) > distance);
        match position {
            Some(index) => self.frames.insert(index, BufferedFrame { frame }),
            None => self.frames.push_back(BufferedFrame { frame }),
        }
    }

    pub(super) fn reset_playout_window(&mut self, sequence: u32, arrival_ms: u64) {
        self.frames.clear();
        self.next_sequence = Some(sequence);
        self.last_played = None;
        self.arrival_anchor = Some((sequence, arrival_ms));
        self.first_arrival_ms = Some(arrival_ms);
        self.next_playout_ms = Some(arrival_ms.saturating_add(self.target_delay_ms));
    }

    pub(super) fn late_frames(&self, sequence: u32, arrival_ms: u64) -> u32 {
        let Some((anchor_sequence, anchor_arrival_ms)) = self.arrival_anchor else {
            return 0;
        };
        let distance = sequence.wrapping_sub(anchor_sequence);
        if distance >= HALF_SEQUENCE_RANGE {
            return 0;
        }
        let expected_arrival_ms =
            anchor_arrival_ms.saturating_add(u64::from(distance).saturating_mul(FRAME_DURATION_MS));
        let late_by_ms = arrival_ms.saturating_sub(expected_arrival_ms);
        if late_by_ms <= LATE_TOLERANCE_MS {
            return 0;
        }
        let late_frames = late_by_ms / FRAME_DURATION_MS;
        u32::try_from(late_frames.max(1).min(u64::from(u32::MAX))).map_or(u32::MAX, |value| value)
    }

    pub(super) fn register_adverse(&mut self, frames: u32) {
        self.stable_frames = 0;
        self.adverse_frames = self.adverse_frames.saturating_add(frames);
        let growth_steps = self.adverse_frames / GROWTH_THRESHOLD_FRAMES;
        self.adverse_frames %= GROWTH_THRESHOLD_FRAMES;
        let growth_ms = u64::from(growth_steps).saturating_mul(TARGET_GROWTH_STEP_MS);
        self.target_delay_ms = self
            .target_delay_ms
            .saturating_add(growth_ms)
            .min(MAX_TARGET_DELAY_MS);
        self.extend_start_deadline();
    }

    pub(super) fn register_stable(&mut self) {
        self.adverse_frames = 0;
        self.stable_frames = self.stable_frames.saturating_add(1);
        if self.stable_frames < STABLE_PERIOD_FRAMES {
            return;
        }
        self.stable_frames = 0;
        self.target_delay_ms = self
            .target_delay_ms
            .saturating_sub(TARGET_DECAY_STEP_MS)
            .max(MIN_TARGET_DELAY_MS);
    }

    pub(super) fn extend_start_deadline(&mut self) {
        let (Some(first_arrival_ms), Some(next_playout_ms)) =
            (self.first_arrival_ms, self.next_playout_ms)
        else {
            return;
        };
        let target_deadline = first_arrival_ms.saturating_add(self.target_delay_ms);
        self.next_playout_ms = Some(next_playout_ms.max(target_deadline));
    }
}
