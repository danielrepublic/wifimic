use std::path::{Path, PathBuf};
use std::time::Duration;

use wifimic_update::{RollbackOutcome, TransactionError, UpdateAdapter, UpdateError};

use crate::updater::TaskSnapshot;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum FailurePoint {
    PreSwap,
    Swap,
    PostSwap,
    Health,
    TaskDoesNotReachRunning,
}

#[derive(Debug)]
pub(super) struct FakeUpdaterState {
    pub(super) calls: Vec<&'static str>,
    pub(super) failure: Option<FailurePoint>,
    pub(super) fail_restore: bool,
    pub(super) current_task: TaskSnapshot,
    pub(super) current_executable: Vec<u8>,
    pub(super) task_running: bool,
    backup_executable: Option<Vec<u8>>,
}

#[derive(Debug)]
pub(super) struct FakeUpdateAdapter {
    pub(super) state: FakeUpdaterState,
}

impl FakeUpdateAdapter {
    pub(super) fn with_failure(failure: Option<FailurePoint>) -> Self {
        Self {
            state: FakeUpdaterState {
                calls: Vec::new(),
                failure,
                fail_restore: false,
                current_task: TaskSnapshot::new(
                    "<Task><Enabled>true</Enabled></Task>".to_owned(),
                    true,
                    true,
                ),
                current_executable: b"old-client".to_vec(),
                task_running: true,
                backup_executable: None,
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

    fn fails_at(&self, point: FailurePoint) -> bool {
        self.state.failure == Some(point)
    }

    fn error_for(point: FailurePoint) -> TransactionError {
        match point {
            FailurePoint::PreSwap => TransactionError::PreSwap {
                message: "pre-swap failed".to_owned(),
            },
            FailurePoint::Swap => TransactionError::Swap {
                message: "swap failed".to_owned(),
            },
            FailurePoint::PostSwap => TransactionError::PostSwap {
                message: "post-swap failed".to_owned(),
            },
            FailurePoint::Health => TransactionError::HealthCheck {
                timeout: Duration::from_secs(45),
            },
            FailurePoint::TaskDoesNotReachRunning => TransactionError::HealthCheck {
                timeout: Duration::from_secs(45),
            },
        }
    }
}

impl UpdateAdapter for FakeUpdateAdapter {
    type Snapshot = TaskSnapshot;

    fn discover_latest_tag(&mut self) -> Result<String, UpdateError> {
        self.state.calls.push("discover");
        Ok("v0.2.0".to_owned())
    }

    fn stage(&mut self, _tag: &str) -> Result<PathBuf, TransactionError> {
        self.state.calls.push("stage");
        Ok(PathBuf::from("staged"))
    }

    fn backup(&mut self, _staged: &Path) -> Result<Self::Snapshot, TransactionError> {
        self.state.calls.push("backup");
        self.state.backup_executable = Some(self.state.current_executable.clone());
        Ok(self.state.current_task.clone())
    }

    fn pre_swap(&mut self, _snapshot: &Self::Snapshot) -> Result<(), TransactionError> {
        self.state.calls.push("pre_swap");
        if self.fails_at(FailurePoint::PreSwap) {
            return Err(Self::error_for(FailurePoint::PreSwap));
        }
        self.state.task_running = false;
        Ok(())
    }

    fn swap(&mut self, _staged: &Path, _snapshot: &Self::Snapshot) -> Result<(), TransactionError> {
        self.state.calls.push("swap");
        if self.fails_at(FailurePoint::Swap) {
            return Err(Self::error_for(FailurePoint::Swap));
        }
        self.state.current_executable = b"new-client".to_vec();
        Ok(())
    }

    fn post_swap(&mut self, snapshot: &Self::Snapshot) -> Result<(), TransactionError> {
        self.state.calls.push("post_swap");
        if self.fails_at(FailurePoint::PostSwap) {
            return Err(Self::error_for(FailurePoint::PostSwap));
        }
        self.state.current_task = snapshot.clone();
        if snapshot.enabled() {
            self.state.calls.push("start_task");
            self.state.task_running = !self.fails_at(FailurePoint::TaskDoesNotReachRunning);
        }
        Ok(())
    }

    fn health_check(&mut self, _timeout: Duration) -> Result<bool, TransactionError> {
        self.state.calls.push("health");
        Ok(self.state.task_running && !self.fails_at(FailurePoint::Health))
    }

    fn rollback(&mut self, snapshot: &Self::Snapshot) -> RollbackOutcome {
        self.state.calls.push("rollback");
        if self.state.task_running {
            self.state.calls.push("stop_task");
            self.state.task_running = false;
            self.state.calls.push("wait_until_stopped");
        }
        self.state.calls.push("restore_executable");
        if self.state.fail_restore {
            return RollbackOutcome::VerificationFailed;
        }
        if let Some(backup) = &self.state.backup_executable {
            self.state.current_executable = backup.clone();
        }
        self.state.current_task = snapshot.clone();
        if snapshot.running() {
            self.state.calls.push("restart_original_task");
            self.state.task_running = true;
        }
        RollbackOutcome::Verified
    }

    fn cleanup_staging(&mut self, _staged: &Path) {
        self.state.calls.push("cleanup_staging");
    }

    fn cleanup_backup(&mut self, _snapshot: &Self::Snapshot) {
        self.state.calls.push("cleanup_backup");
    }
}
