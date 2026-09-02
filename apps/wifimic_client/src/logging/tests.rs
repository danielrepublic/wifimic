use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use super::errors::{Clock, RotationSkipReason, RotationWarning};
use super::rotation::rotate_logs_at;
use super::sink::DiagnosticLogSink;
use super::{RETENTION_MAX_AGE, RETENTION_MAX_BYTES};
use wifimic_diagnostics::{EventRecord, EventSink};

static TEMP_DIRECTORY_COUNTER: AtomicU64 = AtomicU64::new(0);
const LOG_HEADER_VERSION: &str = "wifimic-diagnostics-v1";
const LOG_HEADER_TIMESTAMP: &str = "created_at_unix_secs=";

#[derive(Debug, Clone, Copy)]
struct FixedClock(SystemTime);

impl FixedClock {
    fn new(now: SystemTime) -> Self {
        Self(now)
    }
}

impl Clock for FixedClock {
    fn now(&self) -> SystemTime {
        self.0
    }
}

struct TestDirectory {
    path: PathBuf,
}

impl TestDirectory {
    fn new() -> Self {
        let id = TEMP_DIRECTORY_COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "wifimic-client-logging-{}-{id}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).expect("create test log directory");
        Self { path }
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn write_log(path: &Path, created_at: SystemTime, payload_bytes: usize) {
    let seconds = created_at
        .duration_since(UNIX_EPOCH)
        .expect("test timestamp after epoch")
        .as_secs();
    let header = format!("{LOG_HEADER_VERSION} {LOG_HEADER_TIMESTAMP}{seconds}\n");
    let mut contents = Vec::with_capacity(header.len() + payload_bytes);
    contents.extend_from_slice(header.as_bytes());
    contents.resize(contents.len() + payload_bytes, b'x');
    fs::write(path, contents).expect("write test log");
}

#[test]
fn logging_rotation_enforces_age_and_size_limits() {
    // Given
    let directory = TestDirectory::new();
    let now = UNIX_EPOCH + Duration::from_secs(1_000_000);
    write_log(
        &directory.path.join("old.log"),
        now.checked_sub(RETENTION_MAX_AGE + Duration::from_secs(1))
            .expect("old timestamp"),
        1,
    );
    write_log(
        &directory.path.join("large.log"),
        now.checked_sub(Duration::from_secs(1))
            .expect("large timestamp"),
        usize::try_from(RETENTION_MAX_BYTES).expect("test size fits usize"),
    );
    write_log(&directory.path.join("keep.log"), now, 1);

    // When
    let report = rotate_logs_at(
        &directory.path,
        FixedClock::new(now),
        RETENTION_MAX_AGE,
        RETENTION_MAX_BYTES,
    )
    .expect("rotation succeeds");

    // Then
    assert_eq!(report.removed_files, 2);
    assert!(!directory.path.join("old.log").exists());
    assert!(!directory.path.join("large.log").exists());
    assert!(directory.path.join("keep.log").exists());
    assert!(report.retained_bytes <= RETENTION_MAX_BYTES);
}

#[test]
fn corrupt_log_metadata_is_reported_and_does_not_abort_rotation() {
    // Given
    let directory = TestDirectory::new();
    let now = UNIX_EPOCH + Duration::from_secs(1_000_000);
    fs::write(directory.path.join("corrupt.log"), b"not-a-wifimic-log\n")
        .expect("write corrupt log");
    write_log(&directory.path.join("valid.log"), now, 1);

    // When
    let report = rotate_logs_at(
        &directory.path,
        FixedClock::new(now),
        RETENTION_MAX_AGE,
        RETENTION_MAX_BYTES,
    )
    .expect("corrupt metadata is skipped");

    // Then
    assert!(report.warnings.iter().any(|warning| matches!(
        warning,
        RotationWarning::Skipped {
            path,
            reason: RotationSkipReason::CorruptHeader
        } if path.ends_with("corrupt.log")
    )));
    assert!(directory.path.join("corrupt.log").exists());
    assert!(directory.path.join("valid.log").exists());
}

#[test]
fn repeated_rotation_is_idempotent_and_deterministic() {
    // Given
    let directory = TestDirectory::new();
    let now = UNIX_EPOCH + Duration::from_secs(1_000_000);
    write_log(
        &directory.path.join("first.log"),
        now.checked_sub(Duration::from_secs(1))
            .expect("first timestamp"),
        4,
    );
    write_log(&directory.path.join("second.log"), now, 4);
    let one_log_size = fs::metadata(directory.path.join("second.log"))
        .expect("stat retained log")
        .len();

    // When
    let first = rotate_logs_at(
        &directory.path,
        FixedClock::new(now),
        RETENTION_MAX_AGE,
        one_log_size,
    )
    .expect("first rotation succeeds");
    let second = rotate_logs_at(
        &directory.path,
        FixedClock::new(now),
        RETENTION_MAX_AGE,
        one_log_size,
    )
    .expect("second rotation succeeds");

    // Then
    assert_eq!(first.removed_files, 1);
    assert_eq!(second.removed_files, 0);
    assert_eq!(second.retained_bytes, one_log_size);
    assert!(directory.path.join("second.log").exists());
    assert!(!directory.path.join("first.log").exists());
}

#[test]
fn diagnostic_sink_accepts_only_typed_metadata_records() {
    // Given
    let directory = TestDirectory::new();
    let sink = DiagnosticLogSink::open(
        &directory.path,
        FixedClock::new(UNIX_EPOCH + Duration::from_secs(1_000_000)),
    )
    .expect("open diagnostic sink");
    let event = wifimic_diagnostics::Event::CaptureRetry {
        attempt: 1,
        error_kind: wifimic_diagnostics::ErrorClass::TimedOut,
        retry_delay_ms: 25,
    };

    // When
    sink.record(EventRecord::new(42, Some(7), event));

    // Then
    assert!(sink.take_error().is_none());
    let log_path = fs::read_dir(&directory.path)
        .expect("read test log directory")
        .next()
        .expect("diagnostic log exists")
        .expect("read diagnostic entry")
        .path();
    let contents = fs::read_to_string(log_path).expect("read diagnostic log");
    assert!(contents.contains("event=capture_retry"));
    assert!(!contents.contains("pcm"));
    assert!(!contents.contains("payload"));
    assert!(!contents.contains("samples"));
}

#[test]
fn diagnostic_sink_persists_render_startup_retry_exhausted_event() {
    // Given
    let directory = TestDirectory::new();
    let sink = DiagnosticLogSink::open(
        &directory.path,
        FixedClock::new(UNIX_EPOCH + Duration::from_secs(1_000_000)),
    )
    .expect("open diagnostic sink");
    let context = wifimic_diagnostics::EventContext::new(Instant::now(), sink);

    // When
    context.emit(
        Instant::now(),
        wifimic_diagnostics::Event::RenderStartupRetryExhausted {
            attempt_count: 30,
            elapsed_ms: 60_000,
            failure_class: wifimic_diagnostics::RenderStartupFailureClass::EndpointNotFound,
        },
    );

    // Then
    let log_path = fs::read_dir(&directory.path)
        .expect("read test log directory")
        .next()
        .expect("diagnostic log exists")
        .expect("read diagnostic entry")
        .path();
    let contents = fs::read_to_string(log_path).expect("read diagnostic log");
    assert!(contents.contains("event=render_startup_retry_exhausted"));
    assert!(contents.contains("attempt_count=30 elapsed_ms=60000 failure_class=endpoint_not_found"));
}
