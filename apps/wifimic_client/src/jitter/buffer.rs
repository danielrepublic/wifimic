use std::collections::VecDeque;

use wifimic_protocol::{classify_sequence, AudioFrame, SequenceClassification};

use super::types::{FrameInsertOutcome, PlayoutItem, PlayoutPoll};
use super::{FRAME_DURATION_MS, MAX_BUFFERED_FRAMES, MIN_TARGET_DELAY_MS};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct BufferedFrame {
    pub(super) frame: AudioFrame,
}

/// A bounded, sequence-aware PCM FIFO with an adaptive playout target.
#[derive(Debug)]
pub struct JitterBuffer {
    pub(super) frames: VecDeque<BufferedFrame>,
    pub(super) session_id: Option<u64>,
    pub(super) next_sequence: Option<u32>,
    pub(super) latest_received: Option<u32>,
    pub(super) last_played: Option<u32>,
    pub(super) arrival_anchor: Option<(u32, u64)>,
    pub(super) first_arrival_ms: Option<u64>,
    pub(super) next_playout_ms: Option<u64>,
    pub(super) target_delay_ms: u64,
    pub(super) adverse_frames: u32,
    pub(super) stable_frames: u32,
}

impl Default for JitterBuffer {
    fn default() -> Self {
        Self::new()
    }
}

impl JitterBuffer {
    /// Creates an empty buffer at the minimum playout target.
    #[must_use]
    pub fn new() -> Self {
        Self {
            frames: VecDeque::with_capacity(MAX_BUFFERED_FRAMES),
            session_id: None,
            next_sequence: None,
            latest_received: None,
            last_played: None,
            arrival_anchor: None,
            first_arrival_ms: None,
            next_playout_ms: None,
            target_delay_ms: MIN_TARGET_DELAY_MS,
            adverse_frames: 0,
            stable_frames: 0,
        }
    }

    /// Accepts one decoded protocol frame with its injected monotonic arrival time.
    #[must_use]
    pub fn push(&mut self, frame: AudioFrame, arrival_ms: u64) -> FrameInsertOutcome {
        let sequence = frame.sequence;
        let Some(session_id) = self.session_id else {
            self.session_id = Some(frame.session_id);
            self.next_sequence = Some(sequence);
            self.latest_received = Some(sequence);
            self.arrival_anchor = Some((sequence, arrival_ms));
            self.first_arrival_ms = Some(arrival_ms);
            self.next_playout_ms = Some(arrival_ms.saturating_add(self.target_delay_ms));
            self.frames.push_back(BufferedFrame { frame });
            return FrameInsertOutcome::FirstFrame;
        };
        if session_id != frame.session_id {
            return FrameInsertOutcome::SessionMismatch {
                expected: session_id,
                received: frame.session_id,
            };
        }
        if self.is_duplicate(sequence) {
            return FrameInsertOutcome::Duplicate { sequence };
        }

        let late_frames = self.late_frames(sequence, arrival_ms);
        let classification = self
            .latest_received
            .map_or(SequenceClassification::InOrder, |last_seen| {
                classify_sequence(last_seen, sequence)
            });
        match classification {
            SequenceClassification::InOrder => {
                self.latest_received = Some(sequence);
                if late_frames > 0 {
                    self.register_adverse(late_frames);
                    if self.is_outside_window(sequence) {
                        self.reset_playout_window(sequence, arrival_ms);
                    }
                    self.queue_frame(frame);
                    FrameInsertOutcome::Late { sequence }
                } else {
                    self.register_stable();
                    if self.is_outside_window(sequence) {
                        self.reset_playout_window(sequence, arrival_ms);
                    }
                    self.queue_frame(frame);
                    FrameInsertOutcome::InOrder
                }
            }
            SequenceClassification::Gap { missing_frames } => {
                self.latest_received = Some(sequence);
                self.register_adverse(missing_frames.max(1));
                if late_frames > 0 {
                    self.register_adverse(late_frames);
                }
                if self.is_outside_window(sequence) {
                    self.reset_playout_window(sequence, arrival_ms);
                }
                self.queue_frame(frame);
                FrameInsertOutcome::Gap { missing_frames }
            }
            SequenceClassification::LateOrDuplicate => {
                if self.is_future(sequence) {
                    if late_frames > 0 {
                        self.register_adverse(late_frames);
                    }
                    if self.is_outside_window(sequence) {
                        self.reset_playout_window(sequence, arrival_ms);
                    }
                    self.queue_frame(frame);
                    FrameInsertOutcome::Reordered
                } else {
                    self.register_adverse(1);
                    FrameInsertOutcome::Late { sequence }
                }
            }
        }
    }

    /// Returns the current adaptive target delay in milliseconds.
    #[must_use]
    pub const fn target_delay_ms(&self) -> u64 {
        self.target_delay_ms
    }

    /// Returns the next playout deadline in elapsed-origin milliseconds, if any.
    ///
    /// Callers use this to size a bounded wait instead of polling the
    /// renderer-facing `poll` on a fixed short interval.
    #[must_use]
    pub const fn next_playout_ms(&self) -> Option<u64> {
        self.next_playout_ms
    }

    /// Returns the number of resident, not-yet-played FIFO frames.
    #[must_use]
    pub fn buffered_frames(&self) -> usize {
        self.frames.len()
    }

    /// Polls one renderer-facing slot at an injected monotonic time.
    #[must_use]
    pub fn poll(&mut self, now_ms: u64) -> PlayoutPoll {
        let (Some(next_sequence), Some(next_playout_ms), Some(session_id)) =
            (self.next_sequence, self.next_playout_ms, self.session_id)
        else {
            return PlayoutPoll::not_ready();
        };
        if now_ms < next_playout_ms {
            return PlayoutPoll::not_ready();
        }

        self.next_sequence = Some(next_sequence.wrapping_add(1));
        self.next_playout_ms = Some(next_playout_ms.saturating_add(FRAME_DURATION_MS));
        self.last_played = Some(next_sequence);
        let item = match self
            .frames
            .iter()
            .position(|buffered| buffered.frame.sequence == next_sequence)
        {
            Some(index) => match self.frames.remove(index) {
                Some(buffered) => PlayoutItem::audio(buffered.frame),
                None => PlayoutItem::gap(session_id, next_sequence),
            },
            None => PlayoutItem::gap(session_id, next_sequence),
        };
        PlayoutPoll::frame(item)
    }

    /// Clears all session, sequence, queue, and adaptive state.
    pub fn clear(&mut self) {
        self.frames.clear();
        self.session_id = None;
        self.next_sequence = None;
        self.latest_received = None;
        self.last_played = None;
        self.arrival_anchor = None;
        self.first_arrival_ms = None;
        self.next_playout_ms = None;
        self.target_delay_ms = MIN_TARGET_DELAY_MS;
        self.adverse_frames = 0;
        self.stable_frames = 0;
    }
}
