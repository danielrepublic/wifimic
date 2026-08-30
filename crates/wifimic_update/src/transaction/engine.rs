use std::path::{Path, PathBuf};
use std::time::Duration;

use super::{resolve_action, ResolvedAction, UpdateTarget};
use crate::UpdateError;

/// Describes whether platform rollback restored a known-good state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RollbackOutcome {
    /// The prior installed and managed state was restored.
    Verified,
    /// The prior state could not be fully verified after rollback.
    VerificationFailed,
}

/// Describes the observable result of an update transaction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransactionOutcome {
    /// The selected target already matches the installed version.
    NoOp { current: String, latest: String },
    /// The staged release was installed and passed its health check.
    Installed { tag: String },
    /// A failure after backup was followed by a verified rollback.
    RolledBack { cause: Box<TransactionError> },
    /// A failure after backup was followed by an unverified rollback.
    RollbackVerificationFailed { cause: Box<TransactionError> },
}

/// Reports a failure at a named update-transaction boundary.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum TransactionError {
    /// Target discovery or resolution failed.
    #[error(transparent)]
    Update(#[from] UpdateError),
    /// Staging a verified release failed.
    #[error("could not stage update: {message}")]
    Stage { message: String },
    /// Capturing the known-good state failed.
    #[error("could not create update backup: {message}")]
    Backup { message: String },
    /// Preparing managed state for the atomic replacement failed.
    #[error("pre-swap update operation failed: {message}")]
    PreSwap { message: String },
    /// Atomically replacing the installed artifact failed.
    #[error("could not swap update artifact: {message}")]
    Swap { message: String },
    /// Restoring managed state after replacement failed.
    #[error("post-swap update operation failed: {message}")]
    PostSwap { message: String },
    /// The updated program did not become healthy before the timeout.
    #[error("updated program did not become healthy within {timeout:?}")]
    HealthCheck { timeout: Duration },
    /// Querying the updated program's health failed.
    #[error("could not query updated program health: {message}")]
    HealthQuery { message: String },
}

/// Supplies platform mechanics for the shared update transaction.
///
/// `stage` returns a staging *directory*, never a joined executable path; the
/// platform-specific `swap` implementation owns joining its filename. If
/// `stage` or `backup` creates a partial artifact and then returns `Err`, that
/// method must best-effort clean its own partial artifact because the engine
/// has no returned path or snapshot to clean. Once either method returns its
/// value, the engine owns the corresponding cleanup hook.
pub trait UpdateAdapter {
    /// Captures the platform-specific known-good state after staging succeeds.
    type Snapshot;

    /// Discovers the latest public release tag without mutating installed state.
    fn discover_latest_tag(&mut self) -> Result<String, UpdateError>;
    /// Stages a verified release and returns its staging directory.
    fn stage(&mut self, tag: &str) -> Result<PathBuf, TransactionError>;
    /// Captures the known-good state before any managed-state mutation.
    fn backup(&mut self, staged: &Path) -> Result<Self::Snapshot, TransactionError>;
    /// Prepares platform-managed state for replacement.
    fn pre_swap(&mut self, snapshot: &Self::Snapshot) -> Result<(), TransactionError>;
    /// Replaces the installed artifact using the staged directory.
    fn swap(&mut self, staged: &Path, snapshot: &Self::Snapshot) -> Result<(), TransactionError>;
    /// Restores platform-managed state after replacement.
    fn post_swap(&mut self, snapshot: &Self::Snapshot) -> Result<(), TransactionError>;
    /// Reports whether the updated installation is healthy before the timeout.
    fn health_check(&mut self, timeout: Duration) -> Result<bool, TransactionError>;
    /// Attempts to restore the known-good state after a post-backup failure.
    fn rollback(&mut self, snapshot: &Self::Snapshot) -> RollbackOutcome;
    /// Removes the staging directory after it was successfully returned.
    fn cleanup_staging(&mut self, staged: &Path) {
        let _ = std::fs::remove_dir_all(staged);
    }
    /// Removes a successfully-created backup artifact when it is no longer needed.
    fn cleanup_backup(&mut self, _snapshot: &Self::Snapshot) {}
}

/// Runs the common target-resolution, install, health-check, and rollback lifecycle.
///
/// A failure before `backup` makes no mutation to installed or managed state.
/// Temporary staging and backup artifacts are adapter-owned: failed `stage` and
/// `backup` calls clean their own partial artifacts, while this engine calls the
/// cleanup hooks for artifacts that their methods returned successfully.
///
/// # Errors
///
/// Returns direct errors only before a successful backup. Later failures become
/// rollback outcomes so callers can distinguish verified recovery from a state
/// that needs operator attention.
pub fn run_update_transaction<A: UpdateAdapter>(
    adapter: &mut A,
    target: UpdateTarget,
    current_version: &str,
    health_timeout: Duration,
) -> Result<TransactionOutcome, TransactionError> {
    let action = resolve_action(&target, current_version, || adapter.discover_latest_tag())?;
    let target_tag = match action {
        ResolvedAction::NoOp { current, latest } => {
            return Ok(TransactionOutcome::NoOp { current, latest });
        }
        ResolvedAction::Proceed { target_tag } => target_tag,
    };

    let staged = adapter.stage(&target_tag)?;
    let snapshot = match adapter.backup(&staged) {
        Ok(snapshot) => snapshot,
        Err(error) => {
            adapter.cleanup_staging(&staged);
            return Err(error);
        }
    };

    if let Err(cause) = adapter.pre_swap(&snapshot) {
        return finish_rollback(adapter, &staged, &snapshot, cause);
    }
    if let Err(cause) = adapter.swap(&staged, &snapshot) {
        return finish_rollback(adapter, &staged, &snapshot, cause);
    }
    if let Err(cause) = adapter.post_swap(&snapshot) {
        return finish_rollback(adapter, &staged, &snapshot, cause);
    }
    match adapter.health_check(health_timeout) {
        Ok(true) => {
            adapter.cleanup_backup(&snapshot);
            adapter.cleanup_staging(&staged);
            Ok(TransactionOutcome::Installed { tag: target_tag })
        }
        Ok(false) => finish_rollback(
            adapter,
            &staged,
            &snapshot,
            TransactionError::HealthCheck {
                timeout: health_timeout,
            },
        ),
        Err(cause) => finish_rollback(adapter, &staged, &snapshot, cause),
    }
}

fn finish_rollback<A: UpdateAdapter>(
    adapter: &mut A,
    staged: &Path,
    snapshot: &A::Snapshot,
    cause: TransactionError,
) -> Result<TransactionOutcome, TransactionError> {
    let outcome = match adapter.rollback(snapshot) {
        RollbackOutcome::Verified => {
            adapter.cleanup_backup(snapshot);
            TransactionOutcome::RolledBack {
                cause: Box::new(cause),
            }
        }
        RollbackOutcome::VerificationFailed => TransactionOutcome::RollbackVerificationFailed {
            cause: Box::new(cause),
        },
    };
    adapter.cleanup_staging(staged);
    Ok(outcome)
}
