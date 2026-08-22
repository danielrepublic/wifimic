use proptest::prelude::*;
use wifimic_protocol::{AudioFrame, PCM_PAYLOAD_BYTES};

use super::{
    FrameInsertOutcome, JitterBuffer, PlayoutItem, PlayoutPoll, FRAME_DURATION_MS,
    MAX_BUFFERED_FRAMES, MAX_TARGET_DELAY_MS, MIN_TARGET_DELAY_MS,
};

const SESSION_ID: u64 = 7;

fn frame_for(session_id: u64, sequence: u32) -> AudioFrame {
    AudioFrame::new(session_id, sequence, [sequence as u8; PCM_PAYLOAD_BYTES])
}

fn frame(sequence: u32) -> AudioFrame {
    frame_for(SESSION_ID, sequence)
}

#[test]
fn jitter_reorders_frames_and_reports_gap_late_duplicate() {
    let mut buffer = JitterBuffer::new();

    assert_eq!(buffer.push(frame(10), 0), FrameInsertOutcome::FirstFrame);
    assert_eq!(
        buffer.push(frame(12), 10),
        FrameInsertOutcome::Gap { missing_frames: 1 }
    );
    assert_eq!(buffer.push(frame(11), 11), FrameInsertOutcome::Reordered);
    assert_eq!(
        buffer.push(frame(11), 12),
        FrameInsertOutcome::Duplicate { sequence: 11 }
    );
    assert_eq!(buffer.poll(39), PlayoutPoll::not_ready());

    assert_eq!(
        buffer.poll(40),
        PlayoutPoll::frame(PlayoutItem::audio(frame(10)))
    );
    assert_eq!(
        buffer.poll(45),
        PlayoutPoll::frame(PlayoutItem::audio(frame(11)))
    );
    assert_eq!(
        buffer.poll(50),
        PlayoutPoll::frame(PlayoutItem::audio(frame(12)))
    );
    assert_eq!(
        buffer.push(frame(10), 60),
        FrameInsertOutcome::Late { sequence: 10 }
    );
}

#[test]
fn jitter_preserves_wrapping_sequence_order() {
    let mut buffer = JitterBuffer::new();

    assert_eq!(
        buffer.push(frame(u32::MAX), 0),
        FrameInsertOutcome::FirstFrame
    );
    assert_eq!(buffer.push(frame(0), 5), FrameInsertOutcome::InOrder);
    assert_eq!(
        buffer.push(frame(2), 10),
        FrameInsertOutcome::Gap { missing_frames: 1 }
    );
    assert_eq!(buffer.push(frame(1), 11), FrameInsertOutcome::Reordered);

    assert_eq!(
        buffer.poll(40),
        PlayoutPoll::frame(PlayoutItem::audio(frame(u32::MAX)))
    );
    assert_eq!(
        buffer.poll(45),
        PlayoutPoll::frame(PlayoutItem::audio(frame(0)))
    );
    assert_eq!(
        buffer.poll(50),
        PlayoutPoll::frame(PlayoutItem::audio(frame(1)))
    );
    assert_eq!(
        buffer.poll(55),
        PlayoutPoll::frame(PlayoutItem::audio(frame(2)))
    );
}

#[test]
fn jitter_emits_explicit_gap_slots_in_fifo_order() {
    let mut buffer = JitterBuffer::new();
    let _ = buffer.push(frame(0), 0);
    assert_eq!(
        buffer.push(frame(2), 10),
        FrameInsertOutcome::Gap { missing_frames: 1 }
    );

    assert_eq!(
        buffer.poll(40),
        PlayoutPoll::frame(PlayoutItem::audio(frame(0)))
    );
    assert_eq!(
        buffer.poll(45),
        PlayoutPoll::frame(PlayoutItem::gap(SESSION_ID, 1))
    );
    assert_eq!(
        buffer.poll(50),
        PlayoutPoll::frame(PlayoutItem::audio(frame(2)))
    );
}

#[test]
fn jitter_steady_delivery_keeps_floor() {
    let mut buffer = JitterBuffer::new();

    for sequence in 0..512_u32 {
        let outcome = buffer.push(frame(sequence), u64::from(sequence) * 5);
        assert!(matches!(
            outcome,
            FrameInsertOutcome::FirstFrame | FrameInsertOutcome::InOrder
        ));
    }

    assert_eq!(buffer.target_delay_ms(), MIN_TARGET_DELAY_MS);
}

#[test]
fn jitter_rejects_other_sessions_and_clear_resets_state() {
    let mut buffer = JitterBuffer::new();
    let _ = buffer.push(frame(0), 0);

    assert_eq!(
        buffer.push(frame_for(8, 1), 5),
        FrameInsertOutcome::SessionMismatch {
            expected: SESSION_ID,
            received: 8,
        }
    );
    buffer.clear();

    assert_eq!(buffer.target_delay_ms(), MIN_TARGET_DELAY_MS);
    assert_eq!(buffer.buffered_frames(), 0);
    assert_eq!(
        buffer.push(frame_for(8, 1), 5),
        FrameInsertOutcome::FirstFrame
    );
}

#[test]
fn jitter_grows_under_sustained_loss_and_decays_after_recovery() {
    let mut buffer = JitterBuffer::new();
    let _ = buffer.push(frame(0), 0);

    for index in 0..37_u32 {
        let sequence = 2 + index * 2;
        let _ = buffer.push(frame(sequence), u64::from(sequence) * FRAME_DURATION_MS);
    }
    let grown_delay = buffer.target_delay_ms();

    for sequence in 75..=800_u32 {
        let _ = buffer.push(frame(sequence), u64::from(sequence) * FRAME_DURATION_MS);
    }

    assert!(grown_delay > MIN_TARGET_DELAY_MS);
    assert_eq!(buffer.target_delay_ms(), MIN_TARGET_DELAY_MS);
    assert!(buffer.target_delay_ms() <= grown_delay);
}

#[test]
fn jitter_grows_under_sustained_late_arrivals() {
    let mut buffer = JitterBuffer::new();
    let _ = buffer.push(frame(0), 0);

    for sequence in 1..=32_u32 {
        let expected_arrival = u64::from(sequence) * FRAME_DURATION_MS;
        let _ = buffer.push(frame(sequence), expected_arrival + 20);
    }

    assert!(buffer.target_delay_ms() > MIN_TARGET_DELAY_MS);
    assert!(buffer.target_delay_ms() <= MAX_TARGET_DELAY_MS);
}

#[test]
fn jitter_never_exceeds_200ms_ceiling_under_extreme_loss() {
    let mut buffer = JitterBuffer::new();
    let _ = buffer.push(frame(0), 0);

    for index in 0..10_000_u32 {
        let sequence = 2 + index * 2;
        let _ = buffer.push(frame(sequence), u64::from(sequence) * FRAME_DURATION_MS);
        assert!(buffer.target_delay_ms() <= MAX_TARGET_DELAY_MS);
        assert!(buffer.buffered_frames() <= MAX_BUFFERED_FRAMES);
    }

    assert_eq!(buffer.target_delay_ms(), MAX_TARGET_DELAY_MS);
}

#[test]
fn jitter_handles_a_maximal_protocol_gap_without_unbounded_work() {
    let mut buffer = JitterBuffer::new();
    let _ = buffer.push(frame(0), 0);

    let outcome = buffer.push(frame(0x7fff_ffff), 10_000);

    assert!(matches!(
        outcome,
        FrameInsertOutcome::Gap { missing_frames } if missing_frames > 1_000_000_000
    ));
    assert_eq!(buffer.target_delay_ms(), MAX_TARGET_DELAY_MS);
    assert!(buffer.buffered_frames() <= MAX_BUFFERED_FRAMES);
}

proptest! {
    #[test]
    fn jitter_steady_arrivals_keep_floor(frame_count in 1_usize..512) {
        let mut buffer = JitterBuffer::new();

        for sequence in 0..frame_count as u32 {
            let _ = buffer.push(frame(sequence), u64::from(sequence) * FRAME_DURATION_MS);
        }

        prop_assert_eq!(buffer.target_delay_ms(), MIN_TARGET_DELAY_MS);
    }

    #[test]
    fn jitter_bursty_loss_stays_bounded(
        missing_frames in 1_u32..=128,
        burst_count in 1_usize..=128,
    ) {
        let mut buffer = JitterBuffer::new();
        let _ = buffer.push(frame(0), 0);
        let stride = missing_frames.saturating_add(1);

        for burst in 0..burst_count as u32 {
            let sequence = 1 + burst * stride;
            let _ = buffer.push(frame(sequence), u64::from(sequence) * FRAME_DURATION_MS);
        }

        prop_assert!(buffer.target_delay_ms() >= MIN_TARGET_DELAY_MS);
        prop_assert!(buffer.target_delay_ms() <= MAX_TARGET_DELAY_MS);
    }

    #[test]
    fn jitter_bursty_late_arrivals_stay_bounded(
        delays in prop::collection::vec(0_u64..=100, 1..=256),
    ) {
        let mut buffer = JitterBuffer::new();
        let _ = buffer.push(frame(0), 0);

        for (offset, delay) in delays.into_iter().enumerate() {
            let sequence = offset as u32 + 1;
            let expected_arrival = u64::from(sequence) * FRAME_DURATION_MS;
            let _ = buffer.push(frame(sequence), expected_arrival + delay);
        }

        prop_assert!(buffer.target_delay_ms() >= MIN_TARGET_DELAY_MS);
        prop_assert!(buffer.target_delay_ms() <= MAX_TARGET_DELAY_MS);
    }

    #[test]
    fn jitter_bursty_late_arrivals_grow_but_stay_bounded(
        burst_count in 3_usize..=128,
        delay in 6_u64..=100,
    ) {
        let mut buffer = JitterBuffer::new();
        let _ = buffer.push(frame(0), 0);

        for offset in 0..burst_count as u32 {
            let sequence = offset + 1;
            let expected_arrival = u64::from(sequence) * FRAME_DURATION_MS;
            let _ = buffer.push(frame(sequence), expected_arrival + delay);
        }

        prop_assert!(buffer.target_delay_ms() > MIN_TARGET_DELAY_MS);
        prop_assert!(buffer.target_delay_ms() <= MAX_TARGET_DELAY_MS);
    }
}
