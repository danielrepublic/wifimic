use std::path::{Path, PathBuf};
use std::time::Duration;

use wifimic_update::{
    compare_versions, discover_latest_tag, is_release_tag, UpdateError, VersionComparison,
};

pub(crate) const HEALTH_TIMEOUT: Duration = Duration::from_secs(45);

/// Reports a failed manual upgrade or a failed rollback step.
#[derive(Debug, thiserror::Error)]
pub(crate) enum UpgradeError {
    /// The requested release tag is invalid.
    #[error(transparent)]
    InvalidTarget(#[from] UpdateError),
    /// The embedded version could not be compared with the latest tag.
    #[error("cannot determine whether {current:?} needs an upgrade; use --tag explicitly")]
    IndeterminateCurrent { current: String },
    /// The download or archive verification failed.
    #[error("release download failed: {message}")]
    Download { message: String },
    /// The checksum manifest was malformed.
    #[error("release checksum manifest is malformed")]
    InvalidChecksumManifest,
    /// The archive digest differed from the published manifest.
    #[error("release checksum mismatch: expected {expected}, got {actual}")]
    ChecksumMismatch { expected: String, actual: String },
    /// The release archive could not be extracted safely.
    #[error("release archive could not be extracted: {message}")]
    Archive { message: String },
    /// The extracted archive did not contain a usable server binary.
    #[error("release archive does not contain a non-empty wifimic_server binary")]
    MissingBinary,
    /// The current executable path could not be resolved.
    #[error("could not resolve the installed server path: {message}")]
    InstallPath { message: String },
    /// The current binary could not be backed up.
    #[error("could not back up the current server binary: {message}")]
    Backup { message: String },
    /// A service stop operation failed.
    #[error("could not stop wifimic-server: {message}")]
    Stop { message: String },
    /// The binary replacement failed.
    #[error("could not atomically replace wifimic_server: {message}")]
    Swap { message: String },
    /// A service restart operation failed.
    #[error("could not restart wifimic-server: {message}")]
    Restart { message: String },
    /// The restarted service did not become active before the timeout.
    #[error("wifimic-server did not become active within {timeout:?}")]
    HealthCheck { timeout: Duration },
    /// The health-check command could not be invoked.
    #[error("could not query wifimic-server health: {message}")]
    HealthQuery { message: String },
    /// A failure occurred after mutation and rollback did not fully succeed.
    #[error("upgrade failed: {operation}; rollback failed: {rollback}")]
    Rollback {
        operation: Box<Self>,
        rollback: Box<RollbackError>,
    },
    /// A fake or adapter operation failed at a named seam.
    #[cfg(test)]
    #[error("upgrade operation {operation} failed")]
    Operation { operation: &'static str },
}

/// Reports one or both rollback failures without hiding the original failure.
#[derive(Debug, thiserror::Error)]
pub(crate) enum RollbackError {
    /// Restoring the old binary failed.
    #[error("restore failed: {0}")]
    Restore(Box<UpgradeError>),
    /// Restarting the restored service failed.
    #[error("restart failed: {0}")]
    Restart(Box<UpgradeError>),
    /// Both restore and restart failed.
    #[error("restore failed: {restore}; restart failed: {restart}")]
    Both {
        restore: Box<UpgradeError>,
        restart: Box<UpgradeError>,
    },
}

/// Performs the side effects needed by a manual upgrade.
pub(crate) trait UpgradeOperations {
    /// Resolves an explicit tag or discovers the latest release tag.
    fn resolve_target_tag(&mut self, requested: Option<&str>) -> Result<String, UpgradeError>;
    /// Downloads, verifies, and extracts a release archive.
    fn download_and_verify(&mut self, tag: &str) -> Result<PathBuf, UpgradeError>;
    /// Returns the canonical installed binary path.
    fn install_path(&self) -> Result<PathBuf, UpgradeError>;
    /// Backs up the currently installed binary.
    fn backup_current_binary(&mut self, backup_path: &Path) -> Result<(), UpgradeError>;
    /// Stops the user service.
    fn stop_service(&mut self) -> Result<(), UpgradeError>;
    /// Atomically swaps a staged binary into the installed path.
    fn atomic_swap(
        &mut self,
        staged_binary: &Path,
        install_path: &Path,
    ) -> Result<(), UpgradeError>;
    /// Restarts the user service.
    fn restart_service(&mut self) -> Result<(), UpgradeError>;
    /// Waits for the user service to report active.
    fn wait_for_active(&mut self, timeout: Duration) -> Result<bool, UpgradeError>;
    /// Restores the previous binary into the installed path.
    fn restore_backup(
        &mut self,
        backup_path: &Path,
        install_path: &Path,
    ) -> Result<(), UpgradeError>;
}

/// Uses the native GitHub tag resolver while keeping upgrade side effects injectable.
#[derive(Debug, Default)]
pub(crate) struct NativeUpgradeOperations;

impl UpgradeOperations for NativeUpgradeOperations {
    fn resolve_target_tag(&mut self, requested: Option<&str>) -> Result<String, UpgradeError> {
        match requested {
            Some(tag) if is_release_tag(tag) => Ok(tag.to_owned()),
            Some(tag) => Err(UpdateError::InvalidTag {
                tag: tag.to_owned(),
            }
            .into()),
            None => discover_latest_tag().map_err(UpgradeError::from),
        }
    }

    fn download_and_verify(&mut self, tag: &str) -> Result<PathBuf, UpgradeError> {
        crate::upgrade_native::download_and_verify(tag)
    }

    fn install_path(&self) -> Result<PathBuf, UpgradeError> {
        crate::upgrade_native::install_path()
    }

    fn backup_current_binary(&mut self, backup_path: &Path) -> Result<(), UpgradeError> {
        crate::upgrade_native::backup_current_binary(backup_path)
    }

    fn stop_service(&mut self) -> Result<(), UpgradeError> {
        crate::upgrade_native::stop_service()
    }

    fn atomic_swap(
        &mut self,
        staged_binary: &Path,
        install_path: &Path,
    ) -> Result<(), UpgradeError> {
        crate::upgrade_native::atomic_swap(staged_binary, install_path)
    }

    fn restart_service(&mut self) -> Result<(), UpgradeError> {
        crate::upgrade_native::restart_service()
    }

    fn wait_for_active(&mut self, timeout: Duration) -> Result<bool, UpgradeError> {
        crate::upgrade_native::wait_for_active(timeout)
    }

    fn restore_backup(
        &mut self,
        backup_path: &Path,
        install_path: &Path,
    ) -> Result<(), UpgradeError> {
        crate::upgrade_native::restore_backup(backup_path, install_path)
    }
}

/// Describes the result of a manual upgrade attempt.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum UpgradeOutcome {
    /// No mutation occurred because the current version is already sufficient.
    NoOp { current: String, latest: String },
    /// The requested release was installed and health-checked.
    Installed { tag: String },
}

impl UpgradeOutcome {
    pub(crate) fn render(&self) -> String {
        match self {
            Self::NoOp { .. } => "已是最新版本".to_owned(),
            Self::Installed { tag } => format!("已更新至 {tag}"),
        }
    }
}

/// Runs the transactional upgrade workflow against injected operations.
///
/// # Errors
/// Returns the original operation failure, including a typed rollback failure
/// when restoring or restarting the service also fails.
pub(crate) fn run_upgrade<O: UpgradeOperations>(
    operations: &mut O,
    requested: Option<&str>,
    current: &str,
) -> Result<UpgradeOutcome, UpgradeError> {
    if let Some(tag) = requested.filter(|tag| !is_release_tag(tag)) {
        return Err(UpdateError::InvalidTag {
            tag: tag.to_owned(),
        }
        .into());
    }
    let target = operations.resolve_target_tag(requested)?;
    if !is_release_tag(&target) {
        return Err(UpdateError::InvalidTag { tag: target }.into());
    }
    if requested.is_none() {
        match compare_versions(current, &target) {
            VersionComparison::UpToDate | VersionComparison::CurrentNewer => {
                return Ok(UpgradeOutcome::NoOp {
                    current: current.to_owned(),
                    latest: target,
                })
            }
            VersionComparison::Indeterminate => {
                return Err(UpgradeError::IndeterminateCurrent {
                    current: current.to_owned(),
                })
            }
            VersionComparison::UpdateAvailable => {}
        }
    }

    let staged_dir = operations.download_and_verify(&target)?;
    let install_path = operations.install_path()?;
    let backup_path = backup_path();
    operations.backup_current_binary(&backup_path)?;
    let staged_binary = staged_dir.join("wifimic_server");
    if let Err(error) = operations.stop_service() {
        return Err(with_rollback(
            operations,
            error,
            &backup_path,
            &install_path,
        ));
    }
    if let Err(error) = operations.atomic_swap(&staged_binary, &install_path) {
        return Err(with_rollback(
            operations,
            error,
            &backup_path,
            &install_path,
        ));
    }
    if let Err(error) = operations.restart_service() {
        return Err(with_rollback(
            operations,
            error,
            &backup_path,
            &install_path,
        ));
    }
    match operations.wait_for_active(HEALTH_TIMEOUT) {
        Ok(true) => Ok(UpgradeOutcome::Installed { tag: target }),
        Ok(false) => Err(with_rollback(
            operations,
            UpgradeError::HealthCheck {
                timeout: HEALTH_TIMEOUT,
            },
            &backup_path,
            &install_path,
        )),
        Err(error) => Err(with_rollback(
            operations,
            error,
            &backup_path,
            &install_path,
        )),
    }
}

fn with_rollback<O: UpgradeOperations>(
    operations: &mut O,
    operation: UpgradeError,
    backup_path: &Path,
    install_path: &Path,
) -> UpgradeError {
    let restore = operations.restore_backup(backup_path, install_path);
    let restart = operations.restart_service();
    let rollback = match (restore, restart) {
        (Ok(()), Ok(())) => return operation,
        (Err(restore), Ok(())) => RollbackError::Restore(Box::new(restore)),
        (Ok(()), Err(restart)) => RollbackError::Restart(Box::new(restart)),
        (Err(restore), Err(restart)) => RollbackError::Both {
            restore: Box::new(restore),
            restart: Box::new(restart),
        },
    };
    UpgradeError::Rollback {
        operation: Box::new(operation),
        rollback: Box::new(rollback),
    }
}

fn backup_path() -> PathBuf {
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |duration| duration.as_nanos());
    std::env::temp_dir().join(format!(
        "wifimic_server.backup.{}.{timestamp}",
        std::process::id()
    ))
}

#[cfg(test)]
#[path = "upgrade_tests.rs"]
mod tests;
