use std::cmp::Ordering;
use std::fs::{self, File};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use super::errors::{
    Clock, FileOperation, LoggingError, RotationReport, RotationSkipReason, RotationWarning,
};

const LOG_FILE_EXTENSION: &str = "log";
const LOG_HEADER_VERSION: &str = "wifimic-diagnostics-v1";
const LOG_HEADER_TIMESTAMP: &str = "created_at_unix_secs=";

#[derive(Debug, Clone, PartialEq, Eq)]
struct LogEntry {
    path: PathBuf,
    created_at: SystemTime,
    size: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct LogMetadata {
    created_at: SystemTime,
}

/// Rotates one log directory with an injected clock and explicit retention limits.
pub fn rotate_logs_at<C: Clock>(
    directory: &Path,
    clock: C,
    max_age: Duration,
    max_bytes: u64,
) -> Result<RotationReport, LoggingError> {
    create_log_directory(directory)?;
    let now = clock.now();
    let mut report = RotationReport::new();
    let mut entries = collect_log_entries(directory, &mut report)?;
    entries.sort_by(compare_log_entries);

    let mut retained = Vec::with_capacity(entries.len());
    for entry in entries {
        report.examined_files += 1;
        let age = now
            .duration_since(entry.created_at)
            .map_or(Duration::ZERO, |elapsed| elapsed);
        if age > max_age && remove_entry(&entry, &mut report) {
            continue;
        }
        retained.push(entry);
    }

    let mut retained_bytes = retained
        .iter()
        .fold(0_u64, |total, entry| total.saturating_add(entry.size));
    let mut size_retained = Vec::with_capacity(retained.len());
    for entry in retained {
        if retained_bytes > max_bytes && remove_entry(&entry, &mut report) {
            retained_bytes = retained_bytes.saturating_sub(entry.size);
        } else {
            size_retained.push(entry);
        }
    }
    report.retained_bytes = size_retained
        .iter()
        .fold(0_u64, |total, entry| total.saturating_add(entry.size));
    Ok(report)
}

pub(super) fn create_log_directory(directory: &Path) -> Result<(), LoggingError> {
    fs::create_dir_all(directory)
        .map_err(|error| filesystem_error(FileOperation::CreateDirectory, error))
}

fn collect_log_entries(
    directory: &Path,
    report: &mut RotationReport,
) -> Result<Vec<LogEntry>, LoggingError> {
    let directory_entries = fs::read_dir(directory)
        .map_err(|error| filesystem_error(FileOperation::ReadDirectory, error))?;
    let mut entries = Vec::new();
    for directory_entry in directory_entries {
        let directory_entry = match directory_entry {
            Ok(entry) => entry,
            Err(error) => {
                report
                    .warnings
                    .push(RotationWarning::DirectoryEntryUnreadable(error.kind()));
                continue;
            }
        };
        let path = directory_entry.path();
        if path.extension().and_then(|extension| extension.to_str()) != Some(LOG_FILE_EXTENSION) {
            continue;
        }
        let metadata = match fs::symlink_metadata(&path) {
            Ok(metadata) => metadata,
            Err(error) => {
                report.warnings.push(RotationWarning::Skipped {
                    path,
                    reason: RotationSkipReason::MetadataUnreadable(error.kind()),
                });
                continue;
            }
        };
        if !metadata.file_type().is_file() {
            report.warnings.push(RotationWarning::Skipped {
                path,
                reason: RotationSkipReason::NotRegularFile,
            });
            continue;
        }
        let log_metadata = match read_log_metadata(&path) {
            Ok(metadata) => metadata,
            Err(reason) => {
                report
                    .warnings
                    .push(RotationWarning::Skipped { path, reason });
                continue;
            }
        };
        entries.push(LogEntry {
            path,
            created_at: log_metadata.created_at,
            size: metadata.len(),
        });
    }
    Ok(entries)
}

fn read_log_metadata(path: &Path) -> Result<LogMetadata, RotationSkipReason> {
    let file =
        File::open(path).map_err(|error| RotationSkipReason::HeaderUnreadable(error.kind()))?;
    let mut header = String::new();
    let bytes_read = BufReader::new(file)
        .read_line(&mut header)
        .map_err(|error| RotationSkipReason::HeaderUnreadable(error.kind()))?;
    if bytes_read == 0 {
        return Err(RotationSkipReason::CorruptHeader);
    }
    let Some(timestamp) = header
        .strip_suffix('\n')
        .and_then(|line| line.strip_prefix(LOG_HEADER_VERSION))
        .and_then(|line| line.strip_prefix(' '))
        .and_then(|line| line.strip_prefix(LOG_HEADER_TIMESTAMP))
    else {
        return Err(RotationSkipReason::CorruptHeader);
    };
    let Ok(seconds) = timestamp.trim_end_matches('\r').parse::<u64>() else {
        return Err(RotationSkipReason::CorruptHeader);
    };
    let Some(created_at) = UNIX_EPOCH.checked_add(Duration::from_secs(seconds)) else {
        return Err(RotationSkipReason::CorruptHeader);
    };
    Ok(LogMetadata { created_at })
}

fn compare_log_entries(left: &LogEntry, right: &LogEntry) -> Ordering {
    left.created_at.cmp(&right.created_at).then_with(|| {
        left.path
            .to_string_lossy()
            .cmp(&right.path.to_string_lossy())
    })
}

fn remove_entry(entry: &LogEntry, report: &mut RotationReport) -> bool {
    match fs::remove_file(&entry.path) {
        Ok(()) => {
            report.removed_files += 1;
            true
        }
        Err(error) => {
            report.warnings.push(RotationWarning::RemovalFailed {
                path: entry.path.clone(),
                kind: error.kind(),
            });
            false
        }
    }
}

fn filesystem_error(operation: FileOperation, error: std::io::Error) -> LoggingError {
    LoggingError::FileSystem {
        operation,
        kind: error.kind(),
    }
}
