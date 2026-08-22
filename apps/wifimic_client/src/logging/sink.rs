use std::fs::{self, File, OpenOptions};
use std::io::{self, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use wifimic_diagnostics::{EventRecord, EventSink};

use super::errors::{Clock, FileOperation, LoggingError};

const LOG_FILE_PREFIX: &str = "diagnostics";
const LOG_FILE_EXTENSION: &str = "log";
const LOG_HEADER_VERSION: &str = "wifimic-diagnostics-v1";
const LOG_HEADER_TIMESTAMP: &str = "created_at_unix_secs=";
const LOG_FILE_NAME_ATTEMPTS: u32 = 1_000;
const ACTIVE_LOG_MAX_BYTES: u64 = super::errors::RETENTION_MAX_BYTES;

/// A metadata-only file sink for typed diagnostics records.
#[derive(Debug)]
pub struct DiagnosticLogSink {
    state: Mutex<LogFileState>,
}

#[derive(Debug)]
struct LogFileState {
    directory: PathBuf,
    next_suffix: u32,
    writer: BufWriter<File>,
    last_error: Option<LoggingError>,
}

impl DiagnosticLogSink {
    pub(super) fn open<C: Clock>(directory: &Path, clock: C) -> Result<Self, LoggingError> {
        let created_at = clock.now();
        let seconds = created_at
            .duration_since(UNIX_EPOCH)
            .map_err(|_| LoggingError::ClockBeforeUnixEpoch)?
            .as_secs();
        let header = format!("{LOG_HEADER_VERSION} {LOG_HEADER_TIMESTAMP}{seconds}\n");
        let (writer, next_suffix) = open_writer(directory, seconds, 0, &header)?;
        Ok(Self {
            state: Mutex::new(LogFileState {
                directory: directory.to_owned(),
                next_suffix,
                writer,
                last_error: None,
            }),
        })
    }

    /// Takes the last write failure observed by the infallible `EventSink` interface.
    pub fn take_error(&self) -> Option<LoggingError> {
        self.state.lock().ok()?.last_error.clone()
    }
}

impl EventSink for DiagnosticLogSink {
    fn record(&self, record: EventRecord) {
        let Ok(mut state) = self.state.lock() else {
            return;
        };
        let line = format!("record={record}\n");
        let line_size = u64::try_from(line.len()).map_or(u64::MAX, |size| size);
        let current_size = match state.writer.get_ref().metadata() {
            Ok(metadata) => metadata.len(),
            Err(error) => {
                state.last_error = Some(filesystem_error(FileOperation::WriteLog, error));
                return;
            }
        };
        if current_size.saturating_add(line_size) > ACTIVE_LOG_MAX_BYTES {
            let seconds = match SystemTime::now().duration_since(UNIX_EPOCH) {
                Ok(duration) => duration.as_secs(),
                Err(_) => {
                    state.last_error = Some(LoggingError::ClockBeforeUnixEpoch);
                    return;
                }
            };
            let header = format!("{LOG_HEADER_VERSION} {LOG_HEADER_TIMESTAMP}{seconds}\n");
            match open_writer(&state.directory, seconds, state.next_suffix, &header) {
                Ok((writer, next_suffix)) => {
                    state.writer = writer;
                    state.next_suffix = next_suffix;
                }
                Err(error) => {
                    state.last_error = Some(error);
                    return;
                }
            }
        }
        let result = state
            .writer
            .write_all(line.as_bytes())
            .and_then(|_| state.writer.flush());
        if let Err(error) = result {
            state.last_error = Some(filesystem_error(FileOperation::WriteLog, error));
        }
    }
}

fn filesystem_error(operation: FileOperation, error: io::Error) -> LoggingError {
    LoggingError::FileSystem {
        operation,
        kind: error.kind(),
    }
}

fn open_writer(
    directory: &Path,
    seconds: u64,
    start_suffix: u32,
    header: &str,
) -> Result<(BufWriter<File>, u32), LoggingError> {
    for suffix in start_suffix..LOG_FILE_NAME_ATTEMPTS {
        let file_name = match suffix {
            0 => format!("{LOG_FILE_PREFIX}-{seconds}.{LOG_FILE_EXTENSION}"),
            suffix => format!("{LOG_FILE_PREFIX}-{seconds}-{suffix}.{LOG_FILE_EXTENSION}"),
        };
        let path = directory.join(file_name);
        let file = match OpenOptions::new().create_new(true).write(true).open(&path) {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(filesystem_error(FileOperation::OpenLog, error)),
        };
        let mut writer = BufWriter::new(file);
        if let Err(error) = writer
            .write_all(header.as_bytes())
            .and_then(|_| writer.flush())
        {
            let _ = fs::remove_file(path);
            return Err(filesystem_error(FileOperation::WriteLog, error));
        }
        return Ok((writer, suffix.saturating_add(1)));
    }
    Err(LoggingError::LogFileNameExhausted)
}
