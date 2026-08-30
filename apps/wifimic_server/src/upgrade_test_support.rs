use std::path::{Path, PathBuf};
use std::time::Duration;

use wifimic_update::{RollbackOutcome, TransactionError, UpdateAdapter, UpdateError};

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
pub(super) struct FakeUpdateAdapter {
    pub(super) state: FakeUpgradeState,
}

impl FakeUpdateAdapter {
    pub(super) fn with_failure(failure: Option<FailurePoint>) -> Self {
        Self {
            state: FakeUpgradeState {
                failure,
                ..FakeUpgradeState::default()
            },
        }
    }

    pub(super) fn count(&self, operation: &'static str) -> usize {
        self.state
            .calls
            .iter()
            .filter(|called| **called == operation)
            .count()
    }

    fn failed(&self, point: FailurePoint) -> bool {
        self.state.failure == Some(point)
    }
}

impl UpdateAdapter for FakeUpdateAdapter {
    type Snapshot = PathBuf;

    fn discover_latest_tag(&mut self) -> Result<String, UpdateError> {
        self.state.calls.push("discover");
        Ok("v0.2.0".to_owned())
    }

    fn stage(&mut self, _tag: &str) -> Result<PathBuf, TransactionError> {
        self.state.calls.push("stage");
        Ok(PathBuf::from("/staged"))
    }

    fn backup(&mut self, _staged: &Path) -> Result<Self::Snapshot, TransactionError> {
        self.state.calls.push("backup");
        Ok(PathBuf::from("/backup"))
    }

    fn pre_swap(&mut self, _snapshot: &Self::Snapshot) -> Result<(), TransactionError> {
        self.state.calls.push("stop");
        if self.failed(FailurePoint::Stop) {
            return Err(TransactionError::PreSwap {
                message: "stop".to_owned(),
            });
        }
        Ok(())
    }

    fn swap(&mut self, _staged: &Path, _snapshot: &Self::Snapshot) -> Result<(), TransactionError> {
        self.state.calls.push("swap");
        if self.failed(FailurePoint::Swap) {
            return Err(TransactionError::Swap {
                message: "swap".to_owned(),
            });
        }
        Ok(())
    }

    fn post_swap(&mut self, _snapshot: &Self::Snapshot) -> Result<(), TransactionError> {
        self.state.calls.push("restart");
        if self.failed(FailurePoint::Restart) {
            return Err(TransactionError::PostSwap {
                message: "restart".to_owned(),
            });
        }
        Ok(())
    }

    fn health_check(&mut self, _timeout: Duration) -> Result<bool, TransactionError> {
        self.state.calls.push("health");
        Ok(!self.failed(FailurePoint::Health))
    }

    fn rollback(&mut self, _snapshot: &Self::Snapshot) -> RollbackOutcome {
        self.state.calls.push("restore");
        self.state.calls.push("restart");
        if self.state.fail_restore || self.state.fail_rollback_restart {
            RollbackOutcome::VerificationFailed
        } else {
            RollbackOutcome::Verified
        }
    }

    fn cleanup_staging(&mut self, _staged: &Path) {
        self.state.calls.push("cleanup_staging");
    }

    fn cleanup_backup(&mut self, _snapshot: &Self::Snapshot) {
        self.state.calls.push("cleanup_backup");
    }
}
