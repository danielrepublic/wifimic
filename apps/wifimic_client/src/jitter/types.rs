use wifimic_protocol::AudioFrame;

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
