/// Classifies a received sequence relative to the latest sequence in one session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SequenceClassification {
    /// The received sequence is the immediate wrapping successor.
    InOrder,
    /// One or more later sequences were skipped before this packet arrived.
    Gap {
        /// The number of frames missing between the two sequence values.
        missing_frames: u32,
    },
    /// The packet is a duplicate or lies behind the latest sequence.
    LateOrDuplicate,
}

/// Classifies a sequence using wrapping serial-number arithmetic.
#[must_use]
pub const fn classify_sequence(last_seen: u32, received: u32) -> SequenceClassification {
    const HALF_SEQUENCE_RANGE: u32 = 1_u32 << 31;

    let advance = received.wrapping_sub(last_seen);
    if advance == 1 {
        SequenceClassification::InOrder
    } else if advance > 1 && advance < HALF_SEQUENCE_RANGE {
        SequenceClassification::Gap {
            missing_frames: advance - 1,
        }
    } else {
        SequenceClassification::LateOrDuplicate
    }
}
