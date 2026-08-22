use std::io;
use std::path::PathBuf;
use std::time::{Duration, SystemTime};

use thiserror::Error;

/// Maximum age of a retained diagnostic log.
pub const RETENTION_MAX_AGE: Duration = Duration::from_secs(7 * 24 * 60 * 60);

/// Maximum aggregate size of retained diagnostic logs.
pub const RETENTION_MAX_BYTES: u64 = 10 * 1024 * 1024;

/// A filesystem operation used by a typed logging error.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileOperation {
    CreateDirectory,
    ReadDirectory,
    OpenLog,
    WriteLog,
    RemoveLog,
}

impl std::fmt::Display for FileOperation {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let name = match self {
            Self::CreateDirectory => "create_directory",
            Self::ReadDirectory => "read_directory",
            Self::OpenLog => "open_log",
            Self::WriteLog => "write_log",
            Self::RemoveLog => "remove_log",
        };
        formatter.write_str(name)
    }
}

/// A typed failure from diagnostic logging setup or an explicit rotation call.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum LoggingError {
    #[error("LOCALAPPDATA is unavailable")]
    LocalAppDataUnavailable,
    #[error("{operation} failed: {kind}")]
    FileSystem {
        operation: FileOperation,
        kind: io::ErrorKind,
    },
    #[error("diagnostic clock is before the Unix epoch")]
    ClockBeforeUnixEpoch,
    #[error("no diagnostic log file name was available")]
    LogFileNameExhausted,
}

/// A clock used by rotation and log-file creation.
pub trait Clock {
    /// Returns the current wall-clock time.
    fn now(&self) -> SystemTime;
}

/// The production system wall clock.
#[derive(Debug, Clone, Copy, Default)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> SystemTime {
        SystemTime::now()
    }
}

/// The reason a log entry was skipped without stopping rotation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RotationSkipReason {
    MetadataUnreadable(io::ErrorKind),
    NotRegularFile,
    HeaderUnreadable(io::ErrorKind),
    CorruptHeader,
}

/// A typed warning emitted by rotation while continuing with other entries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RotationWarning {
    Skipped {
        path: PathBuf,
        reason: RotationSkipReason,
    },
    RemovalFailed {
        path: PathBuf,
        kind: io::ErrorKind,
    },
    DirectoryEntryUnreadable(io::ErrorKind),
}

/// Observable outcome of one rotation pass.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RotationReport {
    pub examined_files: usize,
    pub removed_files: usize,
    pub retained_bytes: u64,
    pub warnings: Vec<RotationWarning>,
}

impl RotationReport {
    pub(super) fn new() -> Self {
        Self {
            examined_files: 0,
            removed_files: 0,
            retained_bytes: 0,
            warnings: Vec::new(),
        }
    }
}
