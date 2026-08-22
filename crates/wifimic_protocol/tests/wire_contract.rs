use wifimic_protocol::{
    accepts_session_id, classify_sequence, decode_audio_frame, decode_control, encode_audio_frame,
    encode_control, is_newer_session, AudioFrame, ControlMessage, SequenceClassification,
    SessionIdGenerator, SessionOrder, ACK_TAG, AUDIO_HEADER_BYTES, AUDIO_PACKET_BYTES, AUDIO_TAG,
    BITS_PER_SAMPLE, BYTES_PER_SAMPLE, CHANNELS, CONTROL_HEADER_BYTES, DEFAULT_PORT,
    FRAME_DURATION_MS, HEARTBEAT_TAG, PCM_PAYLOAD_BYTES, SAMPLES_PER_FRAME, SAMPLE_RATE_HZ,
    START_TAG, STOP_TAG, WIRE_VERSION,
};

#[test]
fn pcm_payload_bytes_matches_frame_duration_and_sample_format() {
    // Given: the fixed 48 kHz, 5 ms, mono, 16-bit PCM format.
    let expected_samples = 240_usize;
    let expected_bytes_per_sample = 2_usize;

    // When: the frame dimensions are derived from the named format constants.
    let derived_samples = SAMPLE_RATE_HZ as usize * FRAME_DURATION_MS as usize / 1_000;
    let derived_payload = SAMPLES_PER_FRAME * BYTES_PER_SAMPLE;

    // Then: 240 samples occupy 480 bytes; the reference's unexplained 484 is not reused.
    assert_eq!(DEFAULT_PORT, 6_902);
    assert_eq!(WIRE_VERSION, 1);
    assert_eq!(AUDIO_TAG, 0x00);
    assert_eq!(START_TAG, 0x01);
    assert_eq!(HEARTBEAT_TAG, 0x02);
    assert_eq!(STOP_TAG, 0x03);
    assert_eq!(ACK_TAG, 0x04);
    assert_eq!(CHANNELS, 1);
    assert_eq!(BITS_PER_SAMPLE, 16);
    assert_eq!(SAMPLES_PER_FRAME, expected_samples);
    assert_eq!(derived_samples, expected_samples);
    assert_eq!(BYTES_PER_SAMPLE, expected_bytes_per_sample);
    assert_eq!(PCM_PAYLOAD_BYTES, derived_payload);
    assert_eq!(PCM_PAYLOAD_BYTES, 480);
    assert_ne!(PCM_PAYLOAD_BYTES, 484);
}

#[test]
fn audio_round_trip_uses_documented_big_endian_header() {
    // Given: a fixed frame with independently known header fields and payload bytes.
    let frame = AudioFrame::new(
        0x0102_0304_0506_0708,
        0x0A0B_0C0D,
        [0xA5; PCM_PAYLOAD_BYTES],
    );

    // When: the frame is encoded and decoded.
    let wire = encode_audio_frame(&frame);

    // Then: tag, version, session, and sequence use the documented field order and endianness.
    assert_eq!(wire.len(), AUDIO_PACKET_BYTES);
    assert_eq!(
        &wire[..AUDIO_HEADER_BYTES],
        &[
            AUDIO_TAG,
            WIRE_VERSION,
            1,
            2,
            3,
            4,
            5,
            6,
            7,
            8,
            0x0A,
            0x0B,
            0x0C,
            0x0D,
        ]
    );
    assert_eq!(&wire[AUDIO_HEADER_BYTES..], &[0xA5; PCM_PAYLOAD_BYTES]);
    assert_eq!(decode_audio_frame(&wire), Ok(frame));
}

#[test]
fn decode_rejects_truncated_payload() {
    // Given: an audio datagram missing its final PCM byte.
    let mut packet = [0_u8; AUDIO_PACKET_BYTES - 1];
    packet[0] = AUDIO_TAG;
    packet[1] = WIRE_VERSION;

    // When: the truncated datagram is decoded.
    let decoded = decode_audio_frame(&packet);

    // Then: the decoder returns a typed truncation error without indexing past the input.
    assert_eq!(
        decoded,
        Err(wifimic_protocol::ProtocolError::Truncated {
            expected: AUDIO_PACKET_BYTES,
            actual: AUDIO_PACKET_BYTES - 1,
        })
    );
}

#[test]
fn audio_decode_rejects_invalid_version_tag_and_length() {
    // Given: malformed datagrams that each violate one independent wire rule.
    let mut invalid_version = [0_u8; AUDIO_PACKET_BYTES];
    invalid_version[0] = AUDIO_TAG;
    invalid_version[1] = 0xFF;
    let invalid_tag = [0xFF_u8, WIRE_VERSION];
    let mut overlong = [0_u8; AUDIO_PACKET_BYTES + 1];
    overlong[0] = AUDIO_TAG;
    overlong[1] = WIRE_VERSION;

    // When: each malformed datagram is decoded.
    let version_result = decode_audio_frame(&invalid_version);
    let tag_result = decode_audio_frame(&invalid_tag);
    let length_result = decode_audio_frame(&overlong);

    // Then: each malformed datagram produces the matching typed error.
    assert_eq!(
        version_result,
        Err(wifimic_protocol::ProtocolError::InvalidVersion {
            expected: WIRE_VERSION,
            actual: 0xFF,
        })
    );
    assert_eq!(
        tag_result,
        Err(wifimic_protocol::ProtocolError::InvalidTag { actual: 0xFF })
    );
    assert_eq!(
        length_result,
        Err(wifimic_protocol::ProtocolError::InvalidLength {
            expected: AUDIO_PACKET_BYTES,
            actual: AUDIO_PACKET_BYTES + 1,
        })
    );
}

#[test]
fn control_messages_round_trip_for_each_kind() {
    // Given: one independent message of every control kind.
    let messages = [
        ControlMessage::Start {
            session_id: 0x0102_0304_0506_0708,
        },
        ControlMessage::Heartbeat {
            session_id: 0x1112_1314_1516_1718,
        },
        ControlMessage::Stop {
            session_id: 0x2122_2324_2526_2728,
        },
        ControlMessage::Ack {
            session_id: 0x3132_3334_3536_3738,
            acked_kind: START_TAG,
        },
    ];

    for message in messages {
        // When: the control message is encoded and decoded.
        let wire = encode_control(&message);

        // Then: the wire contract preserves the complete typed message.
        assert_eq!(decode_control(&wire), Ok(message));
    }
}

#[test]
fn start_control_layout_is_tag_version_then_big_endian_session() {
    // Given: a Start message with independently known session bytes.
    let message = ControlMessage::Start {
        session_id: 0x0102_0304_0506_0708,
    };

    // When: it is encoded.
    let wire = encode_control(&message);

    // Then: the exact 10-byte Start layout is stable and documented.
    assert_eq!(wire.len(), CONTROL_HEADER_BYTES);
    assert_eq!(wire, vec![START_TAG, WIRE_VERSION, 1, 2, 3, 4, 5, 6, 7, 8]);
}

#[test]
fn ack_control_layout_appends_the_acknowledged_control_tag() {
    // Given: an Ack for a known Start message.
    let message = ControlMessage::Ack {
        session_id: 0x0102_0304_0506_0708,
        acked_kind: START_TAG,
    };

    // When: it is encoded.
    let wire = encode_control(&message);

    // Then: the acknowledged tag follows the fixed tag/version/session header.
    assert_eq!(
        wire,
        vec![ACK_TAG, WIRE_VERSION, 1, 2, 3, 4, 5, 6, 7, 8, START_TAG]
    );
}

#[test]
fn control_decode_rejects_truncated_invalid_tag_and_invalid_length() {
    // Given: control datagrams that violate distinct framing rules.
    let truncated = [START_TAG, WIRE_VERSION];
    let invalid_tag = [0xFF_u8, WIRE_VERSION, 0, 0, 0, 0, 0, 0, 0, 1];
    let overlong = [START_TAG, WIRE_VERSION, 0, 0, 0, 0, 0, 0, 0, 1, 0];
    let invalid_ack = [ACK_TAG, WIRE_VERSION, 0, 0, 0, 0, 0, 0, 0, 1, 0xFF];

    // When: each malformed datagram is decoded.
    let truncated_result = decode_control(&truncated);
    let tag_result = decode_control(&invalid_tag);
    let length_result = decode_control(&overlong);
    let ack_result = decode_control(&invalid_ack);

    // Then: malformed control data never becomes an accepted message.
    assert_eq!(
        truncated_result,
        Err(wifimic_protocol::ProtocolError::Truncated {
            expected: CONTROL_HEADER_BYTES,
            actual: 2,
        })
    );
    assert_eq!(
        tag_result,
        Err(wifimic_protocol::ProtocolError::InvalidTag { actual: 0xFF })
    );
    assert_eq!(
        length_result,
        Err(wifimic_protocol::ProtocolError::InvalidLength {
            expected: CONTROL_HEADER_BYTES,
            actual: CONTROL_HEADER_BYTES + 1,
        })
    );
    assert_eq!(
        ack_result,
        Err(wifimic_protocol::ProtocolError::InvalidTag { actual: 0xFF })
    );
}

#[test]
fn wrapping_sequence_classification_is_reusable() {
    // Given: the last sequence values and their received successors.
    assert_eq!(
        classify_sequence(u32::MAX, 0),
        SequenceClassification::InOrder
    );
    assert_eq!(
        classify_sequence(10, 13),
        SequenceClassification::Gap { missing_frames: 2 }
    );

    // When: a duplicate or late sequence is classified.
    let classification = classify_sequence(10, 5);

    // Then: it is never mistaken for a forward loss.
    assert_eq!(classification, SequenceClassification::LateOrDuplicate);
}

#[test]
fn session_ordering_rejects_replayed_or_stale_session_id() {
    // Given: an endpoint that has accepted session 100.
    let mut ordering = SessionOrder::new();
    assert!(ordering.accept(100));

    // When: the same and an older session are offered.
    let replayed = ordering.accept(100);
    let stale = ordering.accept(99);

    // Then: neither can supersede the high-water mark, while a newer one can.
    assert!(!replayed);
    assert!(!stale);
    assert!(ordering.accept(101));
    assert_eq!(ordering.last_accepted(), Some(101));
    assert!(!is_newer_session(Some(101), 101));
    assert!(!accepts_session_id(Some(101), 100));
}

#[test]
fn session_id_generator_is_strictly_increasing_across_clock_edges() {
    // Given: a generator and a simulated clock seam.
    let mut generator = SessionIdGenerator::new();

    // When: two IDs are requested in one millisecond and then after a backward jump.
    let first = generator.next_id(10_000);
    let same_millisecond = generator.next_id(10_000);
    let backward_clock = generator.next_id(9_000);

    // Then: every issued ID advances independently of wall-clock monotonicity.
    assert_eq!(first, Ok(10_000));
    assert_eq!(same_millisecond, Ok(10_001));
    assert_eq!(backward_clock, Ok(10_002));
    assert_eq!(wifimic_protocol::next_session_id(9_000, 10_002), Ok(10_003));
}
