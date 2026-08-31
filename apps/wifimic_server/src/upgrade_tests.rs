use std::time::Duration;

use wifimic_update::{run_update_transaction, TransactionOutcome, UpdateTarget};

use crate::upgrade_test_support::{FailurePoint, FakeUpdateAdapter};

const HEALTH_TIMEOUT: Duration = Duration::from_secs(45);

#[test]
fn upgrade_installs_and_cleans_each_returned_artifact() {
    // Given
    let mut adapter = FakeUpdateAdapter::with_failure(None);

    // When
    let result = run_update_transaction(
        &mut adapter,
        UpdateTarget::Tag("v0.2.0".to_owned()),
        "v0.1.12",
        HEALTH_TIMEOUT,
    );

    // Then
    assert_eq!(
        result,
        Ok(TransactionOutcome::Installed {
            tag: "v0.2.0".to_owned(),
        })
    );
    assert_eq!(adapter.count("cleanup_backup"), 1);
    assert_eq!(adapter.count("cleanup_staging"), 1);
}

#[test]
fn upgrade_reports_a_verified_rollback_when_stopping_fails() {
    // Given
    let mut adapter = FakeUpdateAdapter::with_failure(Some(FailurePoint::Stop));

    // When
    let result = run_update_transaction(
        &mut adapter,
        UpdateTarget::Tag("v0.2.0".to_owned()),
        "v0.1.12",
        HEALTH_TIMEOUT,
    );

    // Then
    assert!(matches!(
        result,
        Ok(TransactionOutcome::RolledBack { cause })
            if matches!(*cause, wifimic_update::TransactionError::PreSwap { .. })
    ));
    assert_eq!(adapter.count("restore"), 1);
    assert_eq!(adapter.count("restart"), 1);
}

#[test]
fn upgrade_reports_a_verified_rollback_when_swapping_fails() {
    // Given
    let mut adapter = FakeUpdateAdapter::with_failure(Some(FailurePoint::Swap));

    // When
    let result = run_update_transaction(
        &mut adapter,
        UpdateTarget::Tag("v0.2.0".to_owned()),
        "v0.1.12",
        HEALTH_TIMEOUT,
    );

    // Then
    assert!(matches!(
        result,
        Ok(TransactionOutcome::RolledBack { cause })
            if matches!(*cause, wifimic_update::TransactionError::Swap { .. })
    ));
    assert_eq!(adapter.count("restore"), 1);
    assert_eq!(adapter.count("restart"), 1);
}

#[test]
fn upgrade_reports_a_verified_rollback_when_restarting_fails() {
    // Given
    let mut adapter = FakeUpdateAdapter::with_failure(Some(FailurePoint::Restart));

    // When
    let result = run_update_transaction(
        &mut adapter,
        UpdateTarget::Tag("v0.2.0".to_owned()),
        "v0.1.12",
        HEALTH_TIMEOUT,
    );

    // Then
    assert!(matches!(
        result,
        Ok(TransactionOutcome::RolledBack { cause })
            if matches!(*cause, wifimic_update::TransactionError::PostSwap { .. })
    ));
    assert_eq!(adapter.count("restore"), 1);
    assert_eq!(adapter.count("restart"), 2);
}

#[test]
fn upgrade_reports_a_verified_rollback_when_the_health_check_fails() {
    // Given
    let mut adapter = FakeUpdateAdapter::with_failure(Some(FailurePoint::Health));

    // When
    let result = run_update_transaction(
        &mut adapter,
        UpdateTarget::Tag("v0.2.0".to_owned()),
        "v0.1.12",
        HEALTH_TIMEOUT,
    );

    // Then
    assert!(matches!(
        result,
        Ok(TransactionOutcome::RolledBack { cause })
            if matches!(*cause, wifimic_update::TransactionError::HealthCheck { .. })
    ));
    assert_eq!(adapter.count("restore"), 1);
    assert_eq!(adapter.count("restart"), 2);
}

#[test]
fn upgrade_reports_an_unverified_rollback_and_retains_the_backup() {
    // Given
    let mut adapter = FakeUpdateAdapter::with_failure(Some(FailurePoint::Swap));
    adapter.state.fail_restore = true;
    adapter.state.fail_rollback_restart = true;

    // When
    let result = run_update_transaction(
        &mut adapter,
        UpdateTarget::Tag("v0.2.0".to_owned()),
        "v0.1.12",
        HEALTH_TIMEOUT,
    );

    // Then
    assert!(matches!(
        result,
        Ok(TransactionOutcome::RollbackVerificationFailed { cause })
            if matches!(*cause, wifimic_update::TransactionError::Swap { .. })
    ));
    assert_eq!(adapter.count("cleanup_backup"), 0);
    assert_eq!(adapter.count("cleanup_staging"), 1);
}

#[test]
fn upgrade_without_a_tag_is_a_noop_when_latest_matches_current() {
    // Given
    let mut adapter = FakeUpdateAdapter::with_failure(None);

    // When
    let result =
        run_update_transaction(&mut adapter, UpdateTarget::Latest, "v0.2.0", HEALTH_TIMEOUT);

    // Then
    assert_eq!(
        result,
        Ok(TransactionOutcome::NoOp {
            current: "v0.2.0".to_owned(),
            latest: "v0.2.0".to_owned(),
        })
    );
    assert_eq!(adapter.state.calls, ["discover"]);
}

#[test]
fn upgrade_without_a_tag_installs_when_current_is_newer_than_latest() {
    // Given
    let mut adapter = FakeUpdateAdapter::with_failure(None);

    // When
    let result =
        run_update_transaction(&mut adapter, UpdateTarget::Latest, "v0.3.0", HEALTH_TIMEOUT);

    // Then
    assert_eq!(
        result,
        Ok(TransactionOutcome::Installed {
            tag: "v0.2.0".to_owned(),
        })
    );
}

#[test]
fn upgrade_with_an_explicit_tag_equal_to_current_is_a_noop() {
    // Given
    let mut adapter = FakeUpdateAdapter::with_failure(None);

    // When
    let result = run_update_transaction(
        &mut adapter,
        UpdateTarget::Tag("v0.1.12".to_owned()),
        "v0.1.12",
        HEALTH_TIMEOUT,
    );

    // Then
    assert_eq!(
        result,
        Ok(TransactionOutcome::NoOp {
            current: "v0.1.12".to_owned(),
            latest: "v0.1.12".to_owned(),
        })
    );
    assert!(adapter.state.calls.is_empty());
}
