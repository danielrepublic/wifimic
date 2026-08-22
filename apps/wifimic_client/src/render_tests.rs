use std::time::Duration;

#[cfg(target_os = "windows")]
use wifimic_protocol::AudioFrame;

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
fn render_mono_samples_are_fanned_out_to_interleaved_stereo() {
    let mut mono = [0_u8; PCM_PAYLOAD_BYTES];
    mono[0..2].copy_from_slice(&1_i16.to_le_bytes());
    mono[2..4].copy_from_slice(&(-2_i16).to_le_bytes());

    let stereo = mono_to_stereo_bytes(&mono);

    assert_eq!(&stereo[0..4], &[1, 0, 1, 0]);
    assert_eq!(&stereo[4..8], &[254, 255, 254, 255]);
}

#[test]
fn render_undersized_buffer_is_a_typed_capacity_error() {
    let error = validate_buffer_capacity(239, SAMPLES_PER_FRAME as u32)
        .expect_err("one frame short must fail");

    assert!(matches!(
        error,
        RenderError::BufferTooSmall {
            available_frames: 239,
            required_frames: 240
        }
    ));
}

#[test]
fn render_event_timeout_is_a_bounded_typed_error() {
    let error = classify_event_wait(EventWaitOutcome::TimedOut, 100)
        .expect_err("an absent event must fail the bounded wait");

    assert!(matches!(
        error,
        RenderError::EventWaitTimeout {
            wait_timeout_ms: 100
        }
    ));
}

#[test]
fn render_signaled_event_is_accepted() {
    assert!(classify_event_wait(EventWaitOutcome::Signaled, 100).is_ok());
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

    renderer.stop().expect("stop VB-CABLE render");
}
