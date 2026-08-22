mod errors;
mod rotation;
mod sink;

#[cfg(test)]
mod tests;

use std::env;
use std::path::PathBuf;

pub use errors::{
    Clock, FileOperation, LoggingError, RotationReport, RotationSkipReason, RotationWarning,
    SystemClock, RETENTION_MAX_AGE, RETENTION_MAX_BYTES,
};
pub use rotation::rotate_logs_at;
pub use sink::DiagnosticLogSink;

const LOG_DIRECTORY_NAME: &str = "wifimic-client";
const LOGS_DIRECTORY_NAME: &str = "logs";

/// Returns `%LOCALAPPDATA%\wifimic-client\logs` without a fallback path.
pub fn diagnostic_log_directory() -> Result<PathBuf, LoggingError> {
    let local_app_data =
        env::var_os("LOCALAPPDATA").ok_or(LoggingError::LocalAppDataUnavailable)?;
    Ok(PathBuf::from(local_app_data)
        .join(LOG_DIRECTORY_NAME)
        .join(LOGS_DIRECTORY_NAME))
}

/// Rotates the production diagnostic directory during client startup.
pub fn initialize_diagnostics() -> Result<(DiagnosticLogSink, RotationReport), LoggingError> {
    let directory = diagnostic_log_directory()?;
    rotation::create_log_directory(&directory)?;
    let report = rotate_logs_at(
        &directory,
        SystemClock,
        RETENTION_MAX_AGE,
        RETENTION_MAX_BYTES,
    )?;
    let sink = DiagnosticLogSink::open(&directory, SystemClock)?;
    Ok((sink, report))
}

/// Rotates the production diagnostic directory for a future client-loop tick.
pub fn rotate_diagnostic_logs() -> Result<RotationReport, LoggingError> {
    let directory = diagnostic_log_directory()?;
    rotation::create_log_directory(&directory)?;
    rotate_logs_at(
        &directory,
        SystemClock,
        RETENTION_MAX_AGE,
        RETENTION_MAX_BYTES,
    )
}
