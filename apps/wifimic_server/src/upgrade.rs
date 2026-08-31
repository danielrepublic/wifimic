use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use wifimic_update::{
    discover_latest_tag, RollbackOutcome, TransactionError, UpdateAdapter, UpdateError,
};

pub(crate) const HEALTH_TIMEOUT: Duration = Duration::from_secs(45);

/// Supplies Linux-specific mechanics to the shared update transaction.
#[derive(Debug, Default)]
pub(crate) struct LinuxUpdateAdapter;

impl UpdateAdapter for LinuxUpdateAdapter {
    type Snapshot = PathBuf;

    fn discover_latest_tag(&mut self) -> Result<String, UpdateError> {
        discover_latest_tag()
    }

    fn stage(&mut self, tag: &str) -> Result<PathBuf, TransactionError> {
        crate::upgrade_native::download_and_verify(tag)
    }

    fn backup(&mut self, _staged: &Path) -> Result<Self::Snapshot, TransactionError> {
        let install_path = crate::upgrade_native::install_path()
            .map_err(|message| TransactionError::Backup { message })?;
        let backup_path = backup_path();
        crate::upgrade_native::backup_current_binary(&install_path, &backup_path)
            .map_err(|message| TransactionError::Backup { message })?;
        Ok(backup_path)
    }

    fn pre_swap(&mut self, _snapshot: &Self::Snapshot) -> Result<(), TransactionError> {
        crate::upgrade_native::stop_service()
            .map_err(|message| TransactionError::PreSwap { message })
    }

    fn swap(&mut self, staged: &Path, _snapshot: &Self::Snapshot) -> Result<(), TransactionError> {
        let install_path = crate::upgrade_native::install_path()
            .map_err(|message| TransactionError::Swap { message })?;
        crate::upgrade_native::atomic_swap(&staged.join("wifimic_server"), &install_path)
            .map_err(|message| TransactionError::Swap { message })
    }

    fn post_swap(&mut self, _snapshot: &Self::Snapshot) -> Result<(), TransactionError> {
        crate::upgrade_native::restart_service()
            .map_err(|message| TransactionError::PostSwap { message })
    }

    fn health_check(&mut self, timeout: Duration) -> Result<bool, TransactionError> {
        crate::upgrade_native::wait_for_active(timeout)
            .map_err(|message| TransactionError::HealthQuery { message })
    }

    fn rollback(&mut self, snapshot: &Self::Snapshot) -> RollbackOutcome {
        let restored = crate::upgrade_native::install_path()
            .and_then(|install_path| crate::upgrade_native::restore_backup(snapshot, &install_path))
            .is_ok();
        let restarted = crate::upgrade_native::restart_service().is_ok();
        if restored && restarted {
            RollbackOutcome::Verified
        } else {
            RollbackOutcome::VerificationFailed
        }
    }

    fn cleanup_backup(&mut self, snapshot: &Self::Snapshot) {
        let _ = fs::remove_file(snapshot);
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
