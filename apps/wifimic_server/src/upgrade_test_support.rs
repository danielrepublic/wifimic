use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::upgrade::{UpgradeError, UpgradeOperations};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum FailurePoint {
    Stop,
    Swap,
    Restart,
    Health,
}

#[derive(Debug, Default)]
pub(super) struct FakeUpgradeState {
    pub(super) calls: Vec<&'static str>,
    pub(super) failure: Option<FailurePoint>,
    pub(super) fail_restore: bool,
    pub(super) fail_rollback_restart: bool,
}

#[derive(Debug)]
pub(super) struct FakeUpgradeOperations {
    pub(super) state: FakeUpgradeState,
}

impl FakeUpgradeOperations {
    pub(super) fn with_failure(failure: Option<FailurePoint>) -> Self {
        Self {
            state: FakeUpgradeState {
                failure,
                ..FakeUpgradeState::default()
            },
        }
    }

    fn fail_if(&self, point: FailurePoint) -> Result<(), UpgradeError> {
        if self.state.failure == Some(point) {
            Err(UpgradeError::Operation {
                operation: match point {
                    FailurePoint::Stop => "stop",
                    FailurePoint::Swap => "swap",
                    FailurePoint::Restart => "restart",
                    FailurePoint::Health => "health",
                },
            })
        } else {
            Ok(())
        }
    }

    pub(super) fn count(&self, operation: &'static str) -> usize {
        self.state
            .calls
            .iter()
            .filter(|called| **called == operation)
            .count()
    }
}

impl UpgradeOperations for FakeUpgradeOperations {
    fn resolve_target_tag(&mut self, requested: Option<&str>) -> Result<String, UpgradeError> {
        self.state.calls.push("resolve");
        Ok(requested.unwrap_or("v0.2.0").to_owned())
    }

    fn download_and_verify(&mut self, _tag: &str) -> Result<PathBuf, UpgradeError> {
        self.state.calls.push("download");
        Ok(PathBuf::from("/staged"))
    }

    fn install_path(&self) -> Result<PathBuf, UpgradeError> {
        Ok(PathBuf::from("/installed/wifimic_server"))
    }

    fn backup_current_binary(&mut self, _backup_path: &Path) -> Result<(), UpgradeError> {
        self.state.calls.push("backup");
        Ok(())
    }

    fn stop_service(&mut self) -> Result<(), UpgradeError> {
        self.state.calls.push("stop");
        self.fail_if(FailurePoint::Stop)
    }

    fn atomic_swap(
        &mut self,
        _staged_binary: &Path,
        _install_path: &Path,
    ) -> Result<(), UpgradeError> {
        self.state.calls.push("swap");
        self.fail_if(FailurePoint::Swap)
    }

    fn restart_service(&mut self) -> Result<(), UpgradeError> {
        self.state.calls.push("restart");
        if self.state.fail_rollback_restart || self.state.failure == Some(FailurePoint::Restart) {
            return Err(UpgradeError::Operation {
                operation: "restart",
            });
        }
        Ok(())
    }

    fn wait_for_active(&mut self, _timeout: Duration) -> Result<bool, UpgradeError> {
        self.state.calls.push("health");
        if self.state.failure == Some(FailurePoint::Health) {
            Ok(false)
        } else {
            Ok(true)
        }
    }

    fn restore_backup(
        &mut self,
        _backup_path: &Path,
        _install_path: &Path,
    ) -> Result<(), UpgradeError> {
        self.state.calls.push("restore");
        if self.state.fail_restore {
            Err(UpgradeError::Operation {
                operation: "restore",
            })
        } else {
            Ok(())
        }
    }
}
