use std::collections::VecDeque;

use wifimic_protocol::{classify_sequence, AudioFrame, SequenceClassification};

mod adaptation;

/// The protocol frame duration in milliseconds.
pub const FRAME_DURATION_MS: u64 = wifimic_protocol::FRAME_DURATION_MS as u64;
/// The minimum adaptive playout target.
pub const MIN_TARGET_DELAY_MS: u64 = 40;
/// The maximum adaptive playout target.
pub const MAX_TARGET_DELAY_MS: u64 = 200;
/// The number of adverse frames required for one target-delay growth step.
pub const GROWTH_THRESHOLD_FRAMES: u32 = 3;
/// The number of stable frames required for one target-delay decay step.
pub const STABLE_PERIOD_FRAMES: u32 = 20;
/// The target-delay increase applied after each growth threshold.
pub const TARGET_GROWTH_STEP_MS: u64 = 20;
/// The target-delay decrease applied after each stable period.
pub const TARGET_DECAY_STEP_MS: u64 = 5;
/// The arrival-time tolerance before a frame is considered late.
pub const LATE_TOLERANCE_MS: u64 = FRAME_DURATION_MS;
/// The maximum number of resident FIFO slots, derived from the ceiling.
pub const MAX_BUFFERED_FRAMES: usize = 41;

const HALF_SEQUENCE_RANGE: u32 = 1_u32 << 31;

/// The explicit result of accepting or rejecting one decoded PCM frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameInsertOutcome {
    /// The frame established the session and playout sequence.
    FirstFrame,
    /// The frame was the next wrapping sequence and arrived on time.
    InOrder,
    /// The sequence advanced over one or more missing frames.
    Gap {
        /// The number of sequence slots that were absent before this frame.
        missing_frames: u32,
    },
    /// The frame filled a future FIFO slot after a newer frame arrived first.
    Reordered,
    /// The frame was accepted but arrived later than its sequence-time slot.
    Late {
        /// The late frame's wrapping sequence.
        sequence: u32,
    },
    /// The frame was already resident or was the most recently played frame.
    Duplicate {
        /// The duplicated wrapping sequence.
        sequence: u32,
    },
    /// The frame belongs to a session that must be cleared by the control plane.
    SessionMismatch {
        /// The session currently owned by this buffer.
        expected: u64,
        /// The session carried by the rejected frame.
        received: u64,
    },
}

/// The renderer-facing classification of one playout slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayoutKind {
    /// The slot contains decoded PCM.
    Audio,
    /// The slot is missing and must be concealed by the renderer.
    Gap,
}

/// One item handed to a renderer-facing playout consumer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlayoutItem {
    session_id: u64,
    sequence: u32,
    kind: PlayoutKind,
    frame: Option<AudioFrame>,
}

impl PlayoutItem {
    /// Creates an audio playout item.
    #[must_use]
    pub const fn audio(frame: AudioFrame) -> Self {
        Self {
            session_id: frame.session_id,
            sequence: frame.sequence,
            kind: PlayoutKind::Audio,
            frame: Some(frame),
        }
    }

    /// Creates an explicit gap item for the renderer to conceal.
    #[must_use]
    pub const fn gap(session_id: u64, sequence: u32) -> Self {
        Self {
            session_id,
            sequence,
            kind: PlayoutKind::Gap,
            frame: None,
        }
    }

    /// Returns whether the item contains decoded PCM.
    #[must_use]
    pub const fn kind(self) -> PlayoutKind {
        self.kind
    }

    /// Returns the owning session ID.
    #[must_use]
    pub const fn session_id(self) -> u64 {
        self.session_id
    }

    /// Returns the wrapping sequence represented by this item.
    #[must_use]
    pub const fn sequence(self) -> u32 {
        self.sequence
    }

    /// Returns the PCM frame for an audio item, or `None` for a gap.
    #[must_use]
    pub const fn audio_frame(self) -> Option<AudioFrame> {
        self.frame
    }
}

/// The deterministic result of polling the renderer-facing playout interface.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlayoutPoll {
    item: Option<PlayoutItem>,
}

impl PlayoutPoll {
    /// Creates a not-ready result.
    #[must_use]
    pub const fn not_ready() -> Self {
        Self { item: None }
    }

    /// Creates a ready result containing one FIFO slot.
    #[must_use]
    pub const fn frame(item: PlayoutItem) -> Self {
        Self { item: Some(item) }
    }

    /// Returns the ready slot, if the target delay has elapsed.
    #[must_use]
    pub const fn item(self) -> Option<PlayoutItem> {
        self.item
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct BufferedFrame {
    frame: AudioFrame,
}

/// A bounded, sequence-aware PCM FIFO with an adaptive playout target.
#[derive(Debug)]
pub struct JitterBuffer {
    frames: VecDeque<BufferedFrame>,
    session_id: Option<u64>,
    next_sequence: Option<u32>,
    latest_received: Option<u32>,
    last_played: Option<u32>,
    arrival_anchor: Option<(u32, u64)>,
    first_arrival_ms: Option<u64>,
    next_playout_ms: Option<u64>,
    target_delay_ms: u64,
    adverse_frames: u32,
    stable_frames: u32,
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

#[cfg(test)]
mod tests;
