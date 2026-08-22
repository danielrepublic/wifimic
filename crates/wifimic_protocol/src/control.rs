use crate::{
    ProtocolError, ACK_PACKET_BYTES, ACK_TAG, CONTROL_HEADER_BYTES, HEARTBEAT_TAG,
    MESSAGE_PREFIX_BYTES, SESSION_ID_BYTES, START_TAG, STOP_TAG, WIRE_VERSION,
};

/// The four control messages carried on the shared UDP socket.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlMessage {
    /// Requests or establishes a client session.
    Start { session_id: u64 },
    /// Keeps the exact active client session alive.
    Heartbeat { session_id: u64 },
    /// Requests that the active client session stop.
    Stop { session_id: u64 },
    /// Confirms an accepted Start, Heartbeat, or Stop.
    Ack { session_id: u64, acked_kind: u8 },
}

impl ControlMessage {
    const fn tag(self) -> u8 {
        match self {
            Self::Start { .. } => START_TAG,
            Self::Heartbeat { .. } => HEARTBEAT_TAG,
            Self::Stop { .. } => STOP_TAG,
            Self::Ack { .. } => ACK_TAG,
        }
    }

    const fn session_id(self) -> u64 {
        match self {
            Self::Start { session_id }
            | Self::Heartbeat { session_id }
            | Self::Stop { session_id }
            | Self::Ack { session_id, .. } => session_id,
        }
    }
}

/// Encodes a control message in its exact fixed-width wire representation.
#[must_use]
pub fn encode_control(message: &ControlMessage) -> Vec<u8> {
    let capacity = match message {
        ControlMessage::Ack { .. } => ACK_PACKET_BYTES,
        ControlMessage::Start { .. }
        | ControlMessage::Heartbeat { .. }
        | ControlMessage::Stop { .. } => CONTROL_HEADER_BYTES,
    };
    let mut packet = Vec::with_capacity(capacity);
    packet.push(message.tag());
    packet.push(WIRE_VERSION);
    packet.extend_from_slice(&message.session_id().to_be_bytes());
    if let ControlMessage::Ack { acked_kind, .. } = message {
        packet.push(*acked_kind);
    }
    packet
}

/// Decodes one control datagram without accepting unknown tags or lengths.
///
/// # Errors
///
/// Returns a typed error for an invalid tag, version, truncation, or length.
pub fn decode_control(packet: &[u8]) -> Result<ControlMessage, ProtocolError> {
    let Some(&tag) = packet.first() else {
        return Err(ProtocolError::Truncated {
            expected: CONTROL_HEADER_BYTES,
            actual: 0,
        });
    };
    let expected = match tag {
        START_TAG | HEARTBEAT_TAG | STOP_TAG => CONTROL_HEADER_BYTES,
        ACK_TAG => ACK_PACKET_BYTES,
        _ => return Err(ProtocolError::InvalidTag { actual: tag }),
    };
    let Some(&version) = packet.get(1) else {
        return Err(ProtocolError::Truncated {
            expected,
            actual: packet.len(),
        });
    };
    if version != WIRE_VERSION {
        return Err(ProtocolError::InvalidVersion {
            expected: WIRE_VERSION,
            actual: version,
        });
    }
    if packet.len() < expected {
        return Err(ProtocolError::Truncated {
            expected,
            actual: packet.len(),
        });
    }
    if packet.len() > expected {
        return Err(ProtocolError::InvalidLength {
            expected,
            actual: packet.len(),
        });
    }

    let session_start = MESSAGE_PREFIX_BYTES;
    let session_end = session_start + SESSION_ID_BYTES;
    let mut session_bytes = [0_u8; SESSION_ID_BYTES];
    session_bytes.copy_from_slice(&packet[session_start..session_end]);
    let session_id = u64::from_be_bytes(session_bytes);
    match tag {
        START_TAG => Ok(ControlMessage::Start { session_id }),
        HEARTBEAT_TAG => Ok(ControlMessage::Heartbeat { session_id }),
        STOP_TAG => Ok(ControlMessage::Stop { session_id }),
        ACK_TAG => {
            let acked_kind = packet[CONTROL_HEADER_BYTES];
            if !matches!(acked_kind, START_TAG | HEARTBEAT_TAG | STOP_TAG) {
                return Err(ProtocolError::InvalidTag { actual: acked_kind });
            }
            Ok(ControlMessage::Ack {
                session_id,
                acked_kind,
            })
        }
        _ => Err(ProtocolError::InvalidTag { actual: tag }),
    }
}
