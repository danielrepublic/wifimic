use std::time::{Duration, Instant};

use super::CaptureStream;

const AUDIBLE_SAMPLE_MAGNITUDE: u16 = 1_000;

#[cfg(target_os = "windows")]
#[test]
#[ignore = "requires an active WiFiMic stream and a known non-silent Linux source"]
fn live_vb_cable_output_contains_wifimic_audio() {
    let capture = CaptureStream::open().expect("open VB-CABLE Output capture");
    let deadline = Instant::now() + Duration::from_secs(3);
    let mut contains_audible_audio = false;

    while Instant::now() < deadline {
        for packet in capture
            .read_available()
            .expect("read VB-CABLE Output capture")
        {
            contains_audible_audio |= packet
                .samples
                .iter()
                .any(|sample| sample.unsigned_abs() >= AUDIBLE_SAMPLE_MAGNITUDE);
        }
    }

    assert!(
        contains_audible_audio,
        "the active WiFiMic stream must produce audible VB-CABLE Output PCM"
    );
}
