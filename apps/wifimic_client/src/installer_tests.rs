use std::path::PathBuf;

use super::{run_install, run_upgrade, InstallerConfig, InstallerError};
use crate::installer::test_support::{FailurePoint, FakeInstallerOperations};

fn config() -> InstallerConfig {
    InstallerConfig {
        client_executable: PathBuf::from("candidate.exe"),
        render_endpoint: super::RENDER_ENDPOINT.to_owned(),
    }
}

#[test]
fn install_happy_path_mutates_once_without_rollback() {
    // Given
    let mut operations = FakeInstallerOperations::with_failure(None);
    // When
    let result = run_install(&mut operations, &config());
    // Then
    assert!(result.is_ok());
    assert_eq!(operations.count("restore-task"), 0);
}

#[test]
fn upgrade_happy_path_preserves_the_existing_task() {
    // Given
    let mut operations = FakeInstallerOperations::with_failure(None);
    // When
    let result = run_upgrade(&mut operations, &config());
    // Then
    assert!(result.is_ok());
    assert_eq!(operations.count("disable-stop"), 1);
}

#[test]
fn every_post_mutation_failure_restores_all_owned_state() {
    // Given
    for failure in [
        FailurePoint::Swap,
        FailurePoint::Register,
        FailurePoint::Enable,
        FailurePoint::Health,
    ] {
        let mut operations = FakeInstallerOperations::with_failure(Some(failure));
        // When
        let result = run_upgrade(&mut operations, &config());
        // Then
        assert!(result.is_err());
        assert_eq!(operations.count("restore-task"), 1);
        assert_eq!(operations.count("restore-firewall"), 1);
        assert_eq!(operations.count("restore-executable"), 1);
    }
    let mut operations = FakeInstallerOperations::with_failure(Some(FailurePoint::Firewall));
    let result = run_install(&mut operations, &config());
    assert!(result.is_err());
    assert_eq!(operations.count("restore-task"), 1);
    assert_eq!(operations.count("restore-firewall"), 1);
    assert_eq!(operations.count("restore-executable"), 1);
}

#[test]
fn rollback_failure_is_distinct_from_the_original_failure() {
    // Given
    let mut operations = FakeInstallerOperations::with_failure(Some(FailurePoint::Swap));
    operations.fail_restore = true;
    // When
    let result = run_upgrade(&mut operations, &config());
    // Then
    assert!(matches!(result, Err(InstallerError::Rollback { .. })));
}
