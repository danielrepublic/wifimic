use std::time::{Duration, Instant};

use wifimic_protocol::{AudioFrame, BYTES_PER_SAMPLE, PCM_PAYLOAD_BYTES};

use super::fifo::{plan_render_frames, PcmFifo};
use super::*;

#[test]
fn render_exact_configured_endpoint_is_selected() {
    let endpoints = ["Speakers", DEFAULT_RENDER_ENDPOINT];

    assert!(matches!(
        select_endpoint_index(DEFAULT_RENDER_ENDPOINT, &endpoints),
        Ok(1)
    ));
}

#[test]
fn render_misspelled_endpoint_returns_typed_not_found_error() {
    let endpoints = ["Speakers", DEFAULT_RENDER_ENDPOINT];

    let error = select_endpoint_index("CABLE Input (VB-Audio Virtual Cabl)", &endpoints)
        .expect_err("misspelled endpoint must fail");

    assert!(matches!(error, RenderError::EndpointNotFound { .. }));
}

#[test]
fn render_missing_endpoint_never_falls_back_to_first_endpoint() {
    let endpoints = ["Default Speakers", "Headphones"];

    let error = select_endpoint_index(DEFAULT_RENDER_ENDPOINT, &endpoints)
        .expect_err("missing configured endpoint must not select a default");

    assert!(matches!(error, RenderError::EndpointNotFound { .. }));
}

#[test]
fn render_startup_failure_classification_is_safe_and_coarse() {
    use wifimic_diagnostics::RenderStartupFailureClass;

    let endpoint = RenderError::EndpointNotFound {
        expected: "CABLE Input (VB-Audio Virtual Cable)".to_owned(),
        available: vec!["Speakers".to_owned()],
    };
    assert_eq!(
        classify_render_startup_failure(&endpoint),
        RenderStartupFailureClass::EndpointNotFound
    );

    let worker_errors = [
        RenderError::WorkerSpawn {
            source: std::io::Error::other("spawn"),
        },
        RenderError::WorkerPanicked,
        RenderError::WorkerStopped,
        RenderError::WorkerStartupTimedOut {
            startup_timeout_ms: 60_000,
        },
        RenderError::WorkerStartupFailed,
        RenderError::WorkerStatePoisoned,
        RenderError::WorkerFailed {
            details: "boom".to_owned(),
        },
    ];
    for error in worker_errors {
        assert_eq!(
            classify_render_startup_failure(&error),
            RenderStartupFailureClass::WorkerFailure,
            "worker error {error:?} must classify as WorkerFailure"
        );
    }

    let other_errors = [
        RenderError::InvalidEndpointName,
        RenderError::InvalidEventWaitTimeout,
        RenderError::EventWaitTimeout { wait_timeout_ms: 1 },
        RenderError::QueueFull { capacity_frames: 4 },
        RenderError::BufferSizeOverflow {
            frames: 480,
            bytes_per_frame: 4,
        },
        RenderError::UnsupportedPlatform,
    ];
    for error in other_errors {
        assert_eq!(
            classify_render_startup_failure(&error),
            RenderStartupFailureClass::Other,
            "non-startup error {error:?} must classify as Other"
        );
    }
}

#[test]
fn render_empty_endpoint_name_is_invalid_configuration() {
    let error = RenderConfig::new("").expect_err("empty endpoint name must fail");

    assert!(matches!(error, RenderError::InvalidEndpointName));
}

#[test]
fn render_zero_event_wait_is_invalid_configuration() {
    let error = RenderConfig::default()
        .with_event_wait_timeout(Duration::ZERO)
        .expect_err("zero timeout must fail");

    assert!(matches!(error, RenderError::InvalidEventWaitTimeout));
}

#[test]
fn render_mono_samples_are_fanned_out_to_interleaved_stereo_with_gain_applied() {
    let mut mono = [0_u8; PCM_PAYLOAD_BYTES];
    mono[0..2].copy_from_slice(&1_i16.to_le_bytes());
    mono[2..4].copy_from_slice(&(-2_i16).to_le_bytes());

    let stereo = mono_to_stereo_bytes(&mono);

    let gain = i16::try_from(RENDER_GAIN_MULTIPLIER).expect("gain fits i16");
    let expected_first = gain.to_le_bytes();
    let expected_second = (-2_i16 * gain).to_le_bytes();
    assert_eq!(
        &stereo[0..4],
        &[
            expected_first[0],
            expected_first[1],
            expected_first[0],
            expected_first[1]
        ]
    );
    assert_eq!(
        &stereo[4..8],
        &[
            expected_second[0],
            expected_second[1],
            expected_second[0],
            expected_second[1]
        ]
    );
}

#[test]
fn render_gain_saturates_instead_of_wrapping_at_full_scale_amplitude() {
    let mut mono = [0_u8; PCM_PAYLOAD_BYTES];
    mono[0..2].copy_from_slice(&i16::MAX.to_le_bytes());
    mono[2..4].copy_from_slice(&i16::MIN.to_le_bytes());

    let stereo = mono_to_stereo_bytes(&mono);

    let max_bytes = i16::MAX.to_le_bytes();
    let min_bytes = i16::MIN.to_le_bytes();
    assert_eq!(
        &stereo[0..4],
        &[max_bytes[0], max_bytes[1], max_bytes[0], max_bytes[1]]
    );
    assert_eq!(
        &stereo[4..8],
        &[min_bytes[0], min_bytes[1], min_bytes[0], min_bytes[1]]
    );
}

#[test]
fn render_capacity_plan_consumes_all_two_protocol_frames_when_capacity_is_480() {
    let first = test_audio_frame(1, 0x1111);
    let second = test_audio_frame(2, 0x2222);
    let mut fifo = PcmFifo::new(4);
    fifo.push(&first).expect("first frame fits");
    fifo.push(&second).expect("second frame fits");

    let writable = plan_render_frames(480, fifo.queued_device_frames());
    let mut actual = Vec::new();
    fifo.copy_front(writable, &mut actual);
    fifo.discard_front(writable);

    let mut expected = Vec::new();
    expected.extend(mono_to_stereo_bytes(&first.pcm));
    expected.extend(mono_to_stereo_bytes(&second.pcm));
    assert_eq!(writable, 480);
    assert_eq!(actual, expected);
    assert_eq!(fifo.queued_device_frames(), 0);
}

#[test]
fn render_capacity_plan_preserves_order_when_native_capacity_is_not_a_protocol_multiple() {
    let frames = [
        test_audio_frame(1, 0x1111),
        test_audio_frame(2, 0x2222),
        test_audio_frame(3, 0x3333),
    ];
    let mut fifo = PcmFifo::new(4);
    for frame in &frames {
        fifo.push(frame).expect("test frame fits");
    }

    let mut actual = Vec::new();
    for capacity in [300_usize, 180, 240] {
        let capacity_frames = u32::try_from(capacity).expect("test capacity fits u32");
        let writable = plan_render_frames(capacity_frames, fifo.queued_device_frames());
        let mut chunk = Vec::new();
        fifo.copy_front(writable, &mut chunk);
        fifo.discard_front(writable);
        actual.extend(chunk);
        assert_eq!(writable, capacity);
    }

    let mut expected = Vec::new();
    for frame in &frames {
        expected.extend(mono_to_stereo_bytes(&frame.pcm));
    }
    assert_eq!(actual, expected);
    assert_eq!(fifo.queued_device_frames(), 0);
}

#[test]
fn render_pcm_fifo_rejects_a_frame_only_when_bounded_storage_is_full() {
    let mut fifo = PcmFifo::new(2);
    fifo.push(&test_audio_frame(1, 0x1111))
        .expect("first frame fits");
    fifo.push(&test_audio_frame(2, 0x2222))
        .expect("second frame fits");

    let error = fifo
        .push(&test_audio_frame(3, 0x3333))
        .expect_err("a full renderer FIFO must report backpressure");
    assert!(matches!(
        error,
        RenderError::QueueFull { capacity_frames: 2 }
    ));
}

fn test_audio_frame(session_id: u64, sample: i16) -> AudioFrame {
    let mut pcm = [0_u8; PCM_PAYLOAD_BYTES];
    for sample_bytes in pcm.chunks_exact_mut(BYTES_PER_SAMPLE) {
        sample_bytes.copy_from_slice(&sample.to_le_bytes());
    }
    AudioFrame::new(session_id, 0, pcm)
}

#[cfg(target_os = "windows")]
#[test]
#[ignore = "requires the verified VB-CABLE endpoint on the test host"]
fn render_live_enumeration_finds_verified_vb_cable_endpoint() {
    let endpoints = enumerate_render_endpoints().expect("WASAPI endpoint enumeration");
    assert!(endpoints.iter().any(|name| name == DEFAULT_RENDER_ENDPOINT));
}

#[cfg(target_os = "windows")]
#[test]
#[ignore = "requires VB-CABLE and an external loopback capture"]
fn render_live_vb_cable_accepts_one_khz_tone() {
    let renderer = Renderer::open(RenderConfig::vb_cable()).expect("open VB-CABLE render");
    let mut renderer = renderer;
    let mut sequence = 0_u32;
    let submission_started_at = Instant::now();

    for _ in 0..400 {
        let mut pcm = [0_u8; PCM_PAYLOAD_BYTES];
        for (sample_index, sample_bytes) in pcm.chunks_exact_mut(BYTES_PER_SAMPLE).enumerate() {
            let sample_number = usize::try_from(sequence)
                .expect("sequence fits usize")
                .saturating_mul(SAMPLES_PER_FRAME)
                .saturating_add(sample_index);
            let phase = (sample_number as f64) * std::f64::consts::TAU * 1_000.0
                / f64::from(wifimic_protocol::SAMPLE_RATE_HZ);
            let sample = (phase.sin() * 8_000.0) as i16;
            sample_bytes.copy_from_slice(&sample.to_le_bytes());
        }
        renderer
            .render_frame(&AudioFrame::new(1, sequence, pcm))
            .expect("render tone frame");
        sequence = sequence.wrapping_add(1);
    }

    let submission_duration = submission_started_at.elapsed();
    assert!(
        submission_duration < Duration::from_secs(3),
        "two seconds of protocol PCM must enqueue in under three seconds; took {submission_duration:?}"
    );
    renderer.stop().expect("stop VB-CABLE render");
}
