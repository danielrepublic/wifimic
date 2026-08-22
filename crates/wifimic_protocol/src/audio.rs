use crate::{
    ProtocolError, AUDIO_PACKET_BYTES, AUDIO_TAG, MESSAGE_PREFIX_BYTES, PCM_PAYLOAD_BYTES,
    SEQUENCE_BYTES, SESSION_ID_BYTES, WIRE_VERSION,
};

/// A fixed-size 48 kHz mono PCM audio frame and its transport metadata.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AudioFrame {
    /// The wire version carried by the decoded datagram.
    pub version: u8,
    /// The client session that owns this frame.
    pub session_id: u64,
    /// The wrapping frame sequence number.
    pub sequence: u32,
    /// Signed 16-bit little-endian PCM bytes, copied unchanged from the wire.
    pub pcm: [u8; PCM_PAYLOAD_BYTES],
}

impl AudioFrame {
    /// Constructs an audio frame for the current wire version.
    #[must_use]
    pub const fn new(session_id: u64, sequence: u32, pcm: [u8; PCM_PAYLOAD_BYTES]) -> Self {
        Self {
            version: WIRE_VERSION,
            session_id,
            sequence,
            pcm,
        }
    }
}

/// Encodes an audio frame into its exact tag/version/session/sequence/PCM layout.
#[must_use]
pub fn encode_audio_frame(frame: &AudioFrame) -> [u8; AUDIO_PACKET_BYTES] {
    let mut packet = [0_u8; AUDIO_PACKET_BYTES];
    packet[0] = AUDIO_TAG;
    packet[1] = frame.version;
    let session_start = MESSAGE_PREFIX_BYTES;
    let session_end = session_start + SESSION_ID_BYTES;
    packet[session_start..session_end].copy_from_slice(&frame.session_id.to_be_bytes());
    let sequence_end = session_end + SEQUENCE_BYTES;
    packet[session_end..sequence_end].copy_from_slice(&frame.sequence.to_be_bytes());
    packet[sequence_end..].copy_from_slice(&frame.pcm);
    packet
}

/// Decodes one exact audio datagram without panicking on malformed input.
///
/// # Errors
///
/// Returns a typed error for an invalid tag, version, truncation, or length.
pub fn decode_audio_frame(packet: &[u8]) -> Result<AudioFrame, ProtocolError> {
    let Some(&tag) = packet.first() else {
        return Err(ProtocolError::Truncated {
            expected: AUDIO_PACKET_BYTES,
            actual: 0,
        });
    };
    if tag != AUDIO_TAG {
        return Err(ProtocolError::InvalidTag { actual: tag });
    }
    let Some(&version) = packet.get(1) else {
        return Err(ProtocolError::Truncated {
            expected: AUDIO_PACKET_BYTES,
            actual: packet.len(),
        });
    };
    if version != WIRE_VERSION {
        return Err(ProtocolError::InvalidVersion {
            expected: WIRE_VERSION,
            actual: version,
        });
    }
    if packet.len() < AUDIO_PACKET_BYTES {
        return Err(ProtocolError::Truncated {
            expected: AUDIO_PACKET_BYTES,
            actual: packet.len(),
        });
    }
    if packet.len() > AUDIO_PACKET_BYTES {
        return Err(ProtocolError::InvalidLength {
            expected: AUDIO_PACKET_BYTES,
            actual: packet.len(),
        });
    }

    let session_start = MESSAGE_PREFIX_BYTES;
    let session_end = session_start + SESSION_ID_BYTES;
    let sequence_end = session_end + SEQUENCE_BYTES;
    let mut session_bytes = [0_u8; SESSION_ID_BYTES];
    session_bytes.copy_from_slice(&packet[session_start..session_end]);
    let mut sequence_bytes = [0_u8; SEQUENCE_BYTES];
    sequence_bytes.copy_from_slice(&packet[session_end..sequence_end]);
    let mut pcm = [0_u8; PCM_PAYLOAD_BYTES];
    pcm.copy_from_slice(&packet[sequence_end..]);

    Ok(AudioFrame {
        version,
        session_id: u64::from_be_bytes(session_bytes),
        sequence: u32::from_be_bytes(sequence_bytes),
        pcm,
    })
}
