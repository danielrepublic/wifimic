use std::path::{Path, PathBuf};
use std::time::Duration;

use wifimic_update::{RollbackOutcome, TransactionError, UpdateAdapter, UpdateError};

#[derive(Debug, Clone, Copy)]
pub(super) enum FailurePoint {
    Stage,
    Backup,
    PreSwap,
    Swap,
    PostSwap,
    HealthError,
    HealthFalse,
}

#[derive(Debug)]
pub(super) struct FakeAdapter {
    pub(super) calls: Vec<&'static str>,
    failure: Option<FailurePoint>,
    rollback_outcome: RollbackOutcome,
    pub(super) latest: String,
}

impl FakeAdapter {
    pub(super) fn new(failure: Option<FailurePoint>, rollback_outcome: RollbackOutcome) -> Self {
        Self {
            calls: Vec::new(),
            failure,
            rollback_outcome,
            latest: "v2.0.0".to_owned(),
        }
    }

    fn fails_at(&self, failure: FailurePoint) -> bool {
        self.failure
            .is_some_and(|point| std::mem::discriminant(&point) == std::mem::discriminant(&failure))
    }

    pub(super) fn error_for(failure: FailurePoint) -> TransactionError {
        match failure {
            FailurePoint::Stage => TransactionError::Stage {
                message: "stage failed".to_owned(),
            },
            FailurePoint::Backup => TransactionError::Backup {
                message: "backup failed".to_owned(),
            },
            FailurePoint::PreSwap => TransactionError::PreSwap {
                message: "pre-swap failed".to_owned(),
            },
            FailurePoint::Swap => TransactionError::Swap {
                message: "swap failed".to_owned(),
            },
            FailurePoint::PostSwap => TransactionError::PostSwap {
                message: "post-swap failed".to_owned(),
            },
            FailurePoint::HealthError => TransactionError::HealthQuery {
                message: "health query failed".to_owned(),
            },
            FailurePoint::HealthFalse => TransactionError::HealthCheck {
                timeout: Duration::from_secs(30),
            },
        }
    }
}

impl UpdateAdapter for FakeAdapter {
    type Snapshot = PathBuf;

    fn discover_latest_tag(&mut self) -> Result<String, UpdateError> {
        self.calls.push("discover");
        Ok(self.latest.clone())
    }

    fn stage(&mut self, _tag: &str) -> Result<PathBuf, TransactionError> {
        self.calls.push("stage");
        if self.fails_at(FailurePoint::Stage) {
            return Err(Self::error_for(FailurePoint::Stage));
        }
        Ok(PathBuf::from("staged"))
    }

    fn backup(&mut self, _staged: &Path) -> Result<Self::Snapshot, TransactionError> {
        self.calls.push("backup");
        if self.fails_at(FailurePoint::Backup) {
            return Err(Self::error_for(FailurePoint::Backup));
        }
        Ok(PathBuf::from("backup"))
    }

    fn pre_swap(&mut self, _snapshot: &Self::Snapshot) -> Result<(), TransactionError> {
        self.calls.push("pre_swap");
        if self.fails_at(FailurePoint::PreSwap) {
            return Err(Self::error_for(FailurePoint::PreSwap));
        }
        Ok(())
    }

    fn swap(&mut self, _staged: &Path, _snapshot: &Self::Snapshot) -> Result<(), TransactionError> {
        self.calls.push("swap");
        if self.fails_at(FailurePoint::Swap) {
            return Err(Self::error_for(FailurePoint::Swap));
        }
        Ok(())
    }

    fn post_swap(&mut self, _snapshot: &Self::Snapshot) -> Result<(), TransactionError> {
        self.calls.push("post_swap");
        if self.fails_at(FailurePoint::PostSwap) {
            return Err(Self::error_for(FailurePoint::PostSwap));
        }
        Ok(())
    }

    fn health_check(&mut self, _timeout: Duration) -> Result<bool, TransactionError> {
        self.calls.push("health");
        if self.fails_at(FailurePoint::HealthError) {
            return Err(Self::error_for(FailurePoint::HealthError));
        }
        Ok(!self.fails_at(FailurePoint::HealthFalse))
    }

    fn rollback(&mut self, _snapshot: &Self::Snapshot) -> RollbackOutcome {
        self.calls.push("rollback");
        self.rollback_outcome.clone()
    }

    fn cleanup_staging(&mut self, _staged: &Path) {
        self.calls.push("cleanup_staging");
    }

    fn cleanup_backup(&mut self, _snapshot: &Self::Snapshot) {
        self.calls.push("cleanup_backup");
    }
}
