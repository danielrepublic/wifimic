use std::{
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc,
    },
    time::Instant,
};

#[path = "capture_test_support.rs"]
mod support;

use super::{
    CaptureError, CaptureHandle, PAREC_ARGUMENTS, PCM_PAYLOAD_BYTES, PINNED_CAPTURE_SOURCE,
};
use support::{test_handle, ChunkedReader, FakeProcess, ProcessExit, SequenceClock, TestLauncher};

#[test]
fn capture_handle_is_idle_until_explicit_start() {
    // Given: a newly constructed handle and a launcher that counts spawns.
    let spawn_count = Arc::new(AtomicUsize::new(0));
    let launcher = TestLauncher::new(spawn_count.clone(), FakeProcess::empty());
    let handle = test_handle(launcher, Box::new(SequenceClock::new(Vec::new().into())));

    // When: no start command has been issued.
    // Then: capture remains idle and the process launcher was not called.
    assert!(!handle.is_running());
    assert_eq!(spawn_count.load(Ordering::SeqCst), 0);
}

#[test]
fn pinned_parec_arguments_are_exactly_the_verified_source_contract() {
    // Given: the pinned PipeWire source contract.
    let expected = [
        "--raw",
        "--format=s16le",
        "--rate=48000",
        "--channels=1",
        "--latency-msec=5",
        "--process-time-msec=5",
        "--device=alsa_input.pci-0000_00_1b.0.analog-stereo",
    ];

    // When: the production argument list is inspected.
    // Then: every argument, including ordering and the source name, is pinned.
    assert_eq!(PAREC_ARGUMENTS, expected);
    assert_eq!(PINNED_CAPTURE_SOURCE, &PAREC_ARGUMENTS[6][9..]);
}

#[test]
fn reads_exact_frames_across_partial_stdout_chunks() {
    // Given: two frames delivered in boundaries unrelated to the 480-byte frame size.
    let first = [0x11_u8; PCM_PAYLOAD_BYTES];
    let second = [0x22_u8; PCM_PAYLOAD_BYTES];
    let mut bytes = first.to_vec();
    bytes.extend_from_slice(&second);
    let reader = ChunkedReader::new(bytes, [1, 479, 2, 478, 3, 477]);
    let launcher = TestLauncher::new(
        Arc::new(AtomicUsize::new(0)),
        FakeProcess::from_reader(reader, ProcessExit::success()),
    );
    let mut handle = test_handle(launcher, Box::new(SequenceClock::new(Vec::new().into())));

    // When: the handle is explicitly started and two frames are read.
    handle.start().expect("fake capture must start");
    let first_frame = handle.read_frame().expect("first frame must be complete");
    let second_frame = handle.read_frame().expect("second frame must be complete");

    // Then: exact PCM frame boundaries and bytes are preserved.
    assert_eq!(first_frame.pcm, first);
    assert_eq!(second_frame.pcm, second);
}

#[test]
fn acquisition_timestamps_are_attached_and_monotonic_without_sleeping() {
    // Given: stdout frames and an injected monotonic clock with equal timestamps.
    let frame = [0x33_u8; PCM_PAYLOAD_BYTES];
    let mut bytes = frame.to_vec();
    bytes.extend_from_slice(&frame);
    let base = Instant::now();
    let clock = SequenceClock::new([base, base].into_iter().collect());
    let launcher = TestLauncher::new(
        Arc::new(AtomicUsize::new(0)),
        FakeProcess::from_reader(
            ChunkedReader::new(bytes, [480, 480]),
            ProcessExit::success(),
        ),
    );
    let mut handle = test_handle(launcher, Box::new(clock));

    // When: two frames are produced from stdout.
    handle.start().expect("fake capture must start");
    let first = handle.read_frame().expect("first frame must be complete");
    let second = handle.read_frame().expect("second frame must be complete");

    // Then: each timestamp is the injected acquisition instant and ordering never reverses.
    assert_eq!(first.acquired_at, base);
    assert_eq!(second.acquired_at, base);
    assert!(second.acquired_at >= first.acquired_at);
}

#[test]
fn stop_terminates_the_process_and_is_idempotent() {
    // Given: a started fake capture process with an observable stop signal.
    let stopped = Arc::new(AtomicBool::new(false));
    let launcher = TestLauncher::new(
        Arc::new(AtomicUsize::new(0)),
        FakeProcess::with_stop_signal(stopped.clone()),
    );
    let mut handle = test_handle(launcher, Box::new(SequenceClock::new(Vec::new().into())));
    handle.start().expect("fake capture must start");

    // When: stop is called twice.
    handle.stop().expect("first stop must succeed");
    handle.stop().expect("second stop must be harmless");

    // Then: the child was stopped and the handle owns no stale process.
    assert!(stopped.load(Ordering::SeqCst));
    assert!(!handle.is_running());
}

#[test]
fn missing_endpoint_is_a_typed_error_without_default_substitution() {
    // Given: a fake parec process reporting that the pinned source is absent.
    let launcher = TestLauncher::new(
        Arc::new(AtomicUsize::new(0)),
        FakeProcess::from_reader(
            ChunkedReader::new(Vec::new(), []),
            ProcessExit::failure("Source not found: alsa_input.pci-0000_00_1b.0.analog-stereo"),
        ),
    );
    let mut handle = test_handle(launcher, Box::new(SequenceClock::new(Vec::new().into())));

    // When: the handle starts and attempts to acquire its first frame.
    handle.start().expect("fake parec process must start");
    let result = handle.read_frame();

    // Then: the missing pinned endpoint is explicit and no alternate source is selected.
    assert!(matches!(
        result,
        Err(CaptureError::EndpointNotFound { source_name, .. })
            if source_name == PINNED_CAPTURE_SOURCE
    ));
    assert!(!handle.is_running());
}

#[test]
fn partial_and_empty_stdout_are_typed_malformed_input() {
    // Given: a process that emits fewer than one complete frame.
    let launcher = TestLauncher::new(
        Arc::new(AtomicUsize::new(0)),
        FakeProcess::from_reader(
            ChunkedReader::new(vec![0x44, 0x55], [2]),
            ProcessExit::success(),
        ),
    );
    let mut handle = test_handle(launcher, Box::new(SequenceClock::new(Vec::new().into())));

    // When: a frame is requested.
    handle.start().expect("fake capture must start");
    let result = handle.read_frame();

    // Then: the partial frame reports its exact buffered byte count.
    assert!(matches!(
        result,
        Err(CaptureError::StdoutClosed { bytes_read: 2, .. })
    ));
}

#[test]
#[ignore = "requires the verified Linux PipeWire source on arch-daniel"]
fn capture_adapter_starts_and_stops_real_source() {
    // Given: the real Linux PipeWire graph owns the pinned source.
    let mut handle = CaptureHandle::new();

    // When: capture starts and one frame is acquired.
    handle
        .start()
        .expect("parec must start for the verified source");
    let frame = handle
        .read_frame()
        .expect("the verified source must produce a frame");

    // Then: the real source produces one exact frame and stops cleanly.
    assert_eq!(frame.pcm.len(), PCM_PAYLOAD_BYTES);
    handle.stop().expect("parec must stop cleanly");
    assert!(!handle.is_running());
}
