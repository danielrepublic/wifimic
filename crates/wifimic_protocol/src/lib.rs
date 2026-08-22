//! Fixed wire-contract primitives for the WiFi microphone LAN bridge.
//!
//! The protocol uses one bidirectional UDP socket on [`DEFAULT_PORT`]. Every
//! datagram begins with a one-byte message tag followed by the one-byte
//! [`WIRE_VERSION`]. Header integers are unsigned big-endian values. PCM is
//! signed 16-bit little-endian audio, matching the `s16le` capture format;
//! the PCM bytes are otherwise copied unchanged.
//!
//! ## Audio datagram layout
//!
//! An audio datagram is exactly [`AUDIO_PACKET_BYTES`] bytes:
//!
//! | Offset | Bytes | Field | Encoding |
//! | ---: | ---: | --- | --- |
//! | 0 | 1 | message tag | [`AUDIO_TAG`] (`0x00`) |
//! | 1 | 1 | wire version | [`WIRE_VERSION`] |
//! | 2 | 8 | session ID | `u64`, big-endian |
//! | 10 | 4 | sequence | `u32`, big-endian, wrapping |
//! | 14 | 480 | PCM | 240 mono samples, signed 16-bit little-endian |
//!
//! ## Control datagram layout
//!
//! Start, Heartbeat, and Stop are each [`CONTROL_HEADER_BYTES`] bytes:
//! `tag (1) | version (1) | session_id (8)`. Ack adds one byte containing the
//! acknowledged control tag, so it is [`ACK_PACKET_BYTES`] bytes. The control
//! tags are [`START_TAG`], [`HEARTBEAT_TAG`], [`STOP_TAG`], and [`ACK_TAG`].
//!
//! The reference project used a 484-byte PCM constant for a frame whose format
//! math is 240 samples × 2 bytes = 480 bytes. This crate deliberately derives
//! [`PCM_PAYLOAD_BYTES`] from the named sample format and documents no hidden
//! four-byte trailer because no such trailer is part of this wire contract.

mod audio;
mod control;
mod sequence;
mod session;

pub use audio::{decode_audio_frame, encode_audio_frame, AudioFrame};
pub use control::{decode_control, encode_control, ControlMessage};
pub use sequence::{classify_sequence, SequenceClassification};
pub use session::{
    accepts_session_id, is_newer_session, next_session_id, SessionIdError, SessionIdGenerator,
    SessionOrder,
};

/// The fixed PCM sample rate.
pub const SAMPLE_RATE_HZ: u32 = 48_000;
/// The signed PCM sample width.
pub const BITS_PER_SAMPLE: u32 = 16;
/// The number of source and wire channels.
pub const CHANNELS: u16 = 1;
/// The duration of one UDP audio frame.
pub const FRAME_DURATION_MS: u32 = 5;
/// The number of mono samples in one audio frame.
pub const SAMPLES_PER_FRAME: usize = (SAMPLE_RATE_HZ as usize / 1_000) * FRAME_DURATION_MS as usize;
/// The number of PCM bytes occupied by one 16-bit sample.
pub const BYTES_PER_SAMPLE: usize = BITS_PER_SAMPLE as usize / 8;
/// The number of raw PCM bytes in one audio frame.
pub const PCM_PAYLOAD_BYTES: usize = SAMPLES_PER_FRAME * BYTES_PER_SAMPLE;
/// The fixed UDP port for both audio and control datagrams.
pub const DEFAULT_PORT: u16 = 6_902;

/// The current wire version.
pub const WIRE_VERSION: u8 = 1;
/// The tag for an audio datagram.
pub const AUDIO_TAG: u8 = 0x00;
/// The tag for a client Start request.
pub const START_TAG: u8 = 0x01;
/// The tag for a client Heartbeat request.
pub const HEARTBEAT_TAG: u8 = 0x02;
/// The tag for a client Stop request.
pub const STOP_TAG: u8 = 0x03;
/// The tag for a server acknowledgment.
pub const ACK_TAG: u8 = 0x04;

/// The byte width of a session ID on the wire.
pub const SESSION_ID_BYTES: usize = 8;
/// The byte width of a sequence number on the wire.
pub const SEQUENCE_BYTES: usize = 4;
/// The byte width of the tag and version prefix.
pub const MESSAGE_PREFIX_BYTES: usize = 2;
/// The byte width of an audio header before its PCM payload.
pub const AUDIO_HEADER_BYTES: usize = MESSAGE_PREFIX_BYTES + SESSION_ID_BYTES + SEQUENCE_BYTES;
/// The exact byte length of an audio datagram.
pub const AUDIO_PACKET_BYTES: usize = AUDIO_HEADER_BYTES + PCM_PAYLOAD_BYTES;
/// The exact byte length of Start, Heartbeat, and Stop datagrams.
pub const CONTROL_HEADER_BYTES: usize = MESSAGE_PREFIX_BYTES + SESSION_ID_BYTES;
/// The exact byte length of an Ack datagram.
pub const ACK_PACKET_BYTES: usize = CONTROL_HEADER_BYTES + 1;

/// Session IDs are client-issued unsigned millisecond values.
pub type SessionId = u64;

const _: () = assert!(CHANNELS == 1);
const _: () = assert!(SAMPLE_RATE_HZ.is_multiple_of(1_000));
const _: () = assert!(SAMPLES_PER_FRAME == 240);
const _: () = assert!(BYTES_PER_SAMPLE == 2);
const _: () = assert!(PCM_PAYLOAD_BYTES == 480);
const _: () = assert!(AUDIO_PACKET_BYTES == 494);
const _: () = assert!(CONTROL_HEADER_BYTES == 10);
const _: () = assert!(ACK_PACKET_BYTES == 11);

/// Errors returned while decoding malformed protocol input.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProtocolError {
    /// The datagram ended before the required fixed length.
    Truncated {
        /// The required length for the selected message kind.
        expected: usize,
        /// The received datagram length.
        actual: usize,
    },
    /// The datagram carries a version this crate does not understand.
    InvalidVersion {
        /// The supported wire version.
        expected: u8,
        /// The received wire version.
        actual: u8,
    },
    /// The tag is not valid for the decoder or Ack field.
    InvalidTag {
        /// The received unsupported tag.
        actual: u8,
    },
    /// The datagram has bytes missing or trailing beyond its exact length.
    InvalidLength {
        /// The required exact length.
        expected: usize,
        /// The received datagram length.
        actual: usize,
    },
}

impl std::fmt::Display for ProtocolError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Truncated { expected, actual } => {
                write!(
                    formatter,
                    "truncated datagram: expected {expected} bytes, received {actual}"
                )
            }
            Self::InvalidVersion { expected, actual } => write!(
                formatter,
                "invalid wire version: expected {expected}, received {actual}"
            ),
            Self::InvalidTag { actual } => write!(formatter, "invalid message tag: {actual:#04x}"),
            Self::InvalidLength { expected, actual } => write!(
                formatter,
                "invalid datagram length: expected {expected} bytes, received {actual}"
            ),
        }
    }
}

impl std::error::Error for ProtocolError {}
