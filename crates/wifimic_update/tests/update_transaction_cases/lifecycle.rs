use std::time::Duration;

use wifimic_update::{run_update_transaction, RollbackOutcome, TransactionOutcome, UpdateTarget};

use super::support::{FailurePoint, FakeAdapter};

#[test]
fn no_op_skips_staging_backup_and_cleanup() {
    // Given
    let mut adapter = FakeAdapter::new(None, RollbackOutcome::Verified);
    adapter.latest = "v1.0.0".to_owned();

    // When
    let result = run_update_transaction(
        &mut adapter,
        UpdateTarget::Latest,
        "v1.0.0",
        Duration::from_secs(30),
    );

    // Then
    assert_eq!(
        result,
        Ok(TransactionOutcome::NoOp {
            current: "v1.0.0".to_owned(),
            latest: "v1.0.0".to_owned(),
        })
    );
    assert_eq!(adapter.calls, ["discover"]);
}

#[test]
fn installs_after_the_six_transaction_steps_then_cleans_each_artifact_once() {
    // Given
    let mut adapter = FakeAdapter::new(None, RollbackOutcome::Verified);

    // When
    let result = run_update_transaction(
        &mut adapter,
        UpdateTarget::Latest,
        "v1.0.0",
        Duration::from_secs(30),
    );

    // Then
    assert_eq!(
        result,
        Ok(TransactionOutcome::Installed {
            tag: "v2.0.0".to_owned(),
        })
    );
    assert_eq!(
        adapter.calls,
        [
            "discover",
            "stage",
            "backup",
            "pre_swap",
            "swap",
            "post_swap",
            "health",
            "cleanup_backup",
            "cleanup_staging",
        ]
    );
}

#[test]
fn stage_failure_returns_direct_error_without_backup_rollback_or_cleanup_hook() {
    // Given
    let mut adapter = FakeAdapter::new(Some(FailurePoint::Stage), RollbackOutcome::Verified);

    // When
    let result = run_update_transaction(
        &mut adapter,
        UpdateTarget::Latest,
        "v1.0.0",
        Duration::from_secs(30),
    );

    // Then
    assert_eq!(result, Err(FakeAdapter::error_for(FailurePoint::Stage)));
    assert_eq!(adapter.calls, ["discover", "stage"]);
}

#[test]
fn backup_failure_cleans_returned_staging_without_rollback_or_backup_cleanup() {
    // Given
    let mut adapter = FakeAdapter::new(Some(FailurePoint::Backup), RollbackOutcome::Verified);

    // When
    let result = run_update_transaction(
        &mut adapter,
        UpdateTarget::Latest,
        "v1.0.0",
        Duration::from_secs(30),
    );

    // Then
    assert_eq!(result, Err(FakeAdapter::error_for(FailurePoint::Backup)));
    assert_eq!(
        adapter.calls,
        ["discover", "stage", "backup", "cleanup_staging"]
    );
}

fn assert_verified_rollback(failure: FailurePoint, primary_calls: &[&'static str]) {
    // Given
    let mut adapter = FakeAdapter::new(Some(failure), RollbackOutcome::Verified);

    // When
    let result = run_update_transaction(
        &mut adapter,
        UpdateTarget::Latest,
        "v1.0.0",
        Duration::from_secs(30),
    );

    // Then
    assert_eq!(
        result,
        Ok(TransactionOutcome::RolledBack {
            cause: Box::new(FakeAdapter::error_for(failure)),
        })
    );
    let mut expected_calls = vec!["discover", "stage", "backup"];
    expected_calls.extend_from_slice(primary_calls);
    expected_calls.extend_from_slice(&["rollback", "cleanup_backup", "cleanup_staging"]);
    assert_eq!(adapter.calls, expected_calls);
}

#[test]
fn pre_swap_swap_post_swap_and_health_failures_roll_back_verified_state() {
    assert_verified_rollback(FailurePoint::PreSwap, &["pre_swap"]);
    assert_verified_rollback(FailurePoint::Swap, &["pre_swap", "swap"]);
    assert_verified_rollback(FailurePoint::PostSwap, &["pre_swap", "swap", "post_swap"]);
    assert_verified_rollback(
        FailurePoint::HealthError,
        &["pre_swap", "swap", "post_swap", "health"],
    );
    assert_verified_rollback(
        FailurePoint::HealthFalse,
        &["pre_swap", "swap", "post_swap", "health"],
    );
}

#[test]
fn unverified_rollback_retains_backup_but_cleans_staging_once() {
    // Given
    let mut adapter = FakeAdapter::new(
        Some(FailurePoint::Swap),
        RollbackOutcome::VerificationFailed,
    );

    // When
    let result = run_update_transaction(
        &mut adapter,
        UpdateTarget::Latest,
        "v1.0.0",
        Duration::from_secs(30),
    );

    // Then
    assert_eq!(
        result,
        Ok(TransactionOutcome::RollbackVerificationFailed {
            cause: Box::new(FakeAdapter::error_for(FailurePoint::Swap)),
        })
    );
    assert_eq!(
        adapter.calls,
        [
            "discover",
            "stage",
            "backup",
            "pre_swap",
            "swap",
            "rollback",
            "cleanup_staging",
        ]
    );
}
