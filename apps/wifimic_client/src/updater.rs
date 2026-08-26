use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use thiserror::Error;
use wifimic_update::UpdateError;

#[path = "updater_archive.rs"]
mod updater_archive;
pub use updater_archive::download_and_verify_release;

/// Maximum time allowed for the updated client to become healthy.
pub const HEALTH_TIMEOUT: Duration = Duration::from_secs(45);

const CLIENT_EXECUTABLE_NAME: &str = "wifimic_client.exe";

/// Reports an updater operation failure.
#[derive(Debug, Error)]
pub enum UpdaterError {
    /// The release tag resolver rejected or could not resolve a tag.
    #[error(transparent)]
    InvalidTarget(#[from] UpdateError),
    /// The release archive or checksum manifest could not be downloaded.
    #[error("release download failed: {message}")]
    Download { message: String },
    /// The checksum manifest did not contain a valid SHA-256 digest.
    #[error("release checksum manifest is malformed")]
    InvalidChecksumManifest,
    /// The downloaded archive did not match its published digest.
    #[error("release checksum mismatch: expected {expected}, got {actual}")]
    ChecksumMismatch { expected: String, actual: String },
    /// The release archive was malformed or unsafe to extract.
    #[error("release archive could not be extracted: {message}")]
    Archive { message: String },
    /// The extracted archive did not contain a non-empty client executable.
    #[error("release archive does not contain a non-empty wifimic_client.exe")]
    MissingExecutable,
    /// The installed executable path could not be derived.
    #[error("could not resolve the installed client path: {message}")]
    InstallPath { message: String },
    /// The current executable could not be backed up.
    #[error("could not back up the current client executable: {message}")]
    Backup { message: String },
    /// The previous executable could not be restored.
    #[error("could not restore the previous client executable: {message}")]
    Restore { message: String },
    /// A scheduled-task operation failed at a named seam.
    #[error("scheduled task operation {operation} failed: {message}")]
    Task {
        operation: &'static str,
        message: String,
    },
    /// The injected or adapter operation failed at a named seam.
    #[error("updater operation {operation} failed")]
    Operation { operation: &'static str },
    /// The updated executable could not be atomically installed.
    #[error("could not atomically replace wifimic_client.exe: {message}")]
    Swap { message: String },
    /// The expected VB-CABLE render endpoint was not enumerable.
    #[error("the expected render endpoint was not enumerable: {message}")]
    Endpoint { message: String },
    /// The updated client did not become healthy within the timeout.
    #[error("wifimic_client.exe did not become healthy within {timeout:?}")]
    HealthCheck { timeout: Duration },
    /// The health-check operation could not be queried.
    #[error("could not query wifimic_client.exe health: {message}")]
    HealthQuery { message: String },
}

/// Captures the task definition and lifecycle state before an update.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskSnapshot {
    xml: String,
    enabled: bool,
    running: bool,
}

impl TaskSnapshot {
    /// Creates a task snapshot for an adapter or test double.
    #[must_use]
    pub fn new(xml: String, enabled: bool, running: bool) -> Self {
        Self {
            xml,
            enabled,
            running,
        }
    }

    /// Returns the captured task XML.
    #[must_use]
    pub fn xml(&self) -> &str {
        &self.xml
    }

    /// Returns whether the task was enabled before the update.
    #[must_use]
    pub const fn enabled(&self) -> bool {
        self.enabled
    }

    /// Returns whether the task was running before the update.
    #[must_use]
    pub const fn running(&self) -> bool {
        self.running
    }
}

/// Provides the side effects required by the client update transaction.
pub trait UpdaterOperations {
    /// Resolves the latest public release tag.
    fn resolve_latest_tag(&mut self) -> Result<String, UpdaterError>;
    /// Downloads, verifies, and extracts a release archive into staging.
    fn download_and_verify(&mut self, tag: &str) -> Result<PathBuf, UpdaterError>;
    /// Backs up the currently installed executable.
    fn backup_current_executable(&mut self, backup_path: &Path) -> Result<(), UpdaterError>;
    /// Restores the previous executable into the installed path.
    fn restore_executable(
        &mut self,
        backup_path: &Path,
        install_path: &Path,
    ) -> Result<(), UpdaterError>;
    /// Captures the current scheduled-task definition and state.
    fn get_task(&mut self) -> Result<TaskSnapshot, UpdaterError>;
    /// Disables the scheduled task.
    fn disable_task(&mut self) -> Result<(), UpdaterError>;
    /// Stops the scheduled task.
    fn stop_task(&mut self) -> Result<(), UpdaterError>;
    /// Restores the scheduled-task definition and captured enabled state.
    fn restore_task(&mut self, snapshot: &TaskSnapshot) -> Result<(), UpdaterError>;
    /// Enables the scheduled task.
    fn enable_task(&mut self) -> Result<(), UpdaterError>;
    /// Starts the scheduled task.
    fn start_task(&mut self) -> Result<(), UpdaterError>;
    /// Atomically swaps a staged executable into the installed path.
    fn atomic_swap_executable(
        &mut self,
        staged: &Path,
        install_path: &Path,
    ) -> Result<(), UpdaterError>;
    /// Checks that the expected render endpoint is enumerable.
    fn check_render_endpoint_enumerable(&mut self) -> Result<bool, UpdaterError>;
    /// Waits for the updated client to report healthy.
    fn wait_for_healthy(&mut self, timeout: Duration) -> Result<bool, UpdaterError>;
}

/// Describes the result of a client update attempt.
#[derive(Debug, PartialEq, Eq)]
pub enum UpdaterOutcome {
    /// No mutation occurred because the current version is already installed.
    NoOp,
    /// The release was installed and health-checked.
    Installed { tag: String },
    /// The update failed, but the previous executable and task were restored.
    RolledBack,
    /// The update failed and rollback could not be fully verified.
    RollbackVerificationFailed,
}

/// Runs the transactional client update workflow against injected operations.
pub fn run_update<O: UpdaterOperations>(
    operations: &mut O,
    current_version: &str,
) -> Result<UpdaterOutcome, UpdaterError> {
    let target = operations.resolve_latest_tag()?;
    if target == current_version {
        return Ok(UpdaterOutcome::NoOp);
    }

    let staged_directory = operations.download_and_verify(&target)?;
    let install_path = installed_executable_path()?;
    let backup_path = unique_backup_path();
    operations.backup_current_executable(&backup_path)?;
    let snapshot = operations.get_task()?;
    let staged_executable = staged_directory.join(CLIENT_EXECUTABLE_NAME);

    if operations.disable_task().is_err() {
        return Ok(rollback(operations, &backup_path, &install_path, &snapshot));
    }
    if operations.stop_task().is_err() {
        return Ok(rollback(operations, &backup_path, &install_path, &snapshot));
    }
    if operations
        .atomic_swap_executable(&staged_executable, &install_path)
        .is_err()
    {
        return Ok(rollback(operations, &backup_path, &install_path, &snapshot));
    }
    if operations.restore_task(&snapshot).is_err() {
        return Ok(rollback(operations, &backup_path, &install_path, &snapshot));
    }
    if operations.enable_task().is_err() {
        return Ok(rollback(operations, &backup_path, &install_path, &snapshot));
    }
    if snapshot.running() && operations.start_task().is_err() {
        return Ok(rollback(operations, &backup_path, &install_path, &snapshot));
    }
    if !matches!(operations.check_render_endpoint_enumerable(), Ok(true)) {
        return Ok(rollback(operations, &backup_path, &install_path, &snapshot));
    }
    if !matches!(operations.wait_for_healthy(HEALTH_TIMEOUT), Ok(true)) {
        return Ok(rollback(operations, &backup_path, &install_path, &snapshot));
    }

    Ok(UpdaterOutcome::Installed { tag: target })
}

fn rollback<O: UpdaterOperations>(
    operations: &mut O,
    backup_path: &Path,
    install_path: &Path,
    snapshot: &TaskSnapshot,
) -> UpdaterOutcome {
    let executable_restored = operations
        .restore_executable(backup_path, install_path)
        .is_ok();
    let task_restored = operations.restore_task(snapshot).is_ok();
    let task_restarted = !snapshot.running() || operations.start_task().is_ok();
    if executable_restored && task_restored && task_restarted {
        UpdaterOutcome::RolledBack
    } else {
        UpdaterOutcome::RollbackVerificationFailed
    }
}

fn installed_executable_path() -> Result<PathBuf, UpdaterError> {
    std::env::current_exe()
        .map(|path| path.with_file_name(CLIENT_EXECUTABLE_NAME))
        .map_err(|error| UpdaterError::InstallPath {
            message: error.to_string(),
        })
}

fn unique_backup_path() -> PathBuf {
    std::env::temp_dir().join(format!(
        "wifimic_client.backup.{}-{}",
        std::process::id(),
        timestamp()
    ))
}

fn timestamp() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_nanos())
}
