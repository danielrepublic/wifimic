use super::{run_upgrade, UpgradeError, UpgradeOutcome};
use crate::upgrade_test_support::{FailurePoint, FakeUpgradeOperations};

#[test]
fn upgrade_happy_path_replaces_and_health_checks_the_binary() {
    // Given
    let mut operations = FakeUpgradeOperations::with_failure(None);

    // When
    let result =
        run_upgrade(&mut operations, Some("v0.2.0"), "v0.1.12").expect("fake upgrade succeeds");

    // Then
    assert_eq!(
        result,
        UpgradeOutcome::Installed {
            tag: "v0.2.0".to_owned()
        }
    );
    assert_eq!(operations.count("restore"), 0);
    assert_eq!(operations.count("restart"), 1);
}

#[test]
fn upgrade_rolls_back_when_stop_fails() {
    // Given
    let mut operations = FakeUpgradeOperations::with_failure(Some(FailurePoint::Stop));

    // When
    let result = run_upgrade(&mut operations, Some("v0.2.0"), "v0.1.12");

    // Then
    assert!(matches!(
        result,
        Err(UpgradeError::Operation { operation: "stop" })
    ));
    assert_eq!(operations.count("restore"), 1);
    assert_eq!(operations.count("restart"), 1);
}

#[test]
fn upgrade_rolls_back_when_swap_fails() {
    // Given
    let mut operations = FakeUpgradeOperations::with_failure(Some(FailurePoint::Swap));

    // When
    let result = run_upgrade(&mut operations, Some("v0.2.0"), "v0.1.12");

    // Then
    assert!(matches!(
        result,
        Err(UpgradeError::Operation { operation: "swap" })
    ));
    assert_eq!(operations.count("restore"), 1);
    assert_eq!(operations.count("restart"), 1);
}

#[test]
fn upgrade_rolls_back_when_restart_fails() {
    // Given
    let mut operations = FakeUpgradeOperations::with_failure(Some(FailurePoint::Restart));

    // When
    let result = run_upgrade(&mut operations, Some("v0.2.0"), "v0.1.12");

    // Then
    assert!(matches!(result, Err(UpgradeError::Rollback { .. })));
    assert_eq!(operations.count("restore"), 1);
    assert_eq!(operations.count("restart"), 2);
}

#[test]
fn upgrade_rolls_back_when_health_check_fails() {
    // Given
    let mut operations = FakeUpgradeOperations::with_failure(Some(FailurePoint::Health));

    // When
    let result = run_upgrade(&mut operations, Some("v0.2.0"), "v0.1.12");

    // Then
    assert!(matches!(result, Err(UpgradeError::HealthCheck { .. })));
    assert_eq!(operations.count("restore"), 1);
    assert_eq!(operations.count("restart"), 2);
}

#[test]
fn upgrade_reports_rollback_failure_distinctly() {
    // Given
    let mut operations = FakeUpgradeOperations::with_failure(Some(FailurePoint::Swap));
    operations.state.fail_restore = true;
    operations.state.fail_rollback_restart = true;

    // When
    let result = run_upgrade(&mut operations, Some("v0.2.0"), "v0.1.12");

    // Then
    assert!(matches!(
        result,
        Err(UpgradeError::Rollback {
            rollback: error, ..
        }) if matches!(*error, super::RollbackError::Both { .. })
    ));
}

#[test]
fn upgrade_without_tag_is_a_noop_when_latest_matches_current() {
    // Given
    let mut operations = FakeUpgradeOperations::with_failure(None);

    // When
    let result = run_upgrade(&mut operations, None, "v0.2.0").expect("fake check succeeds");

    // Then
    assert_eq!(
        result,
        UpgradeOutcome::NoOp {
            current: "v0.2.0".to_owned(),
            latest: "v0.2.0".to_owned(),
        }
    );
    assert_eq!(operations.count("download"), 0);
    assert_eq!(operations.count("stop"), 0);
    assert_eq!(operations.count("swap"), 0);
    assert_eq!(operations.count("restart"), 0);
}
