mod adaptation;
mod buffer;
mod types;

#[cfg(test)]
mod tests;

pub use buffer::JitterBuffer;
pub use types::{FrameInsertOutcome, PlayoutItem, PlayoutKind, PlayoutPoll};

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

pub(super) const HALF_SEQUENCE_RANGE: u32 = 1_u32 << 31;
