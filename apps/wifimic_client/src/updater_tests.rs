use std::time::Duration;

use wifimic_update::{run_update_transaction, TransactionOutcome, UpdateTarget};

use crate::updater::TaskSnapshot;
use crate::updater_test_support::{FailurePoint, FakeUpdateAdapter};

const HEALTH_TIMEOUT: Duration = Duration::from_secs(45);
const TARGET_VERSION: &str = "v0.2.0";

#[test]
fn explicit_current_target_returns_noop_without_adapter_operations() {
    // Given
    let mut adapter = FakeUpdateAdapter::with_failure(None);

    // When
    let result = run_update_transaction(
        &mut adapter,
        UpdateTarget::Tag(TARGET_VERSION.to_owned()),
        TARGET_VERSION,
        HEALTH_TIMEOUT,
    );

    // Then
    assert_eq!(
        result,
        Ok(TransactionOutcome::NoOp {
            current: TARGET_VERSION.to_owned(),
            latest: TARGET_VERSION.to_owned(),
        })
    );
    assert!(adapter.state.calls.is_empty());
}

#[test]
fn update_installs_and_cleans_returned_artifacts() {
    // Given
    let mut adapter = FakeUpdateAdapter::with_failure(None);

    // When
    let result = run_update_transaction(
        &mut adapter,
        UpdateTarget::Tag(TARGET_VERSION.to_owned()),
        "v0.1.0",
        HEALTH_TIMEOUT,
    );

    // Then
    assert_eq!(
        result,
        Ok(TransactionOutcome::Installed {
            tag: TARGET_VERSION.to_owned(),
        })
    );
    assert_eq!(
        adapter.state.calls,
        [
            "stage",
            "backup",
            "pre_swap",
            "swap",
            "post_swap",
            "start_task",
            "health",
            "cleanup_backup",
            "cleanup_staging",
        ]
    );
}

fn assert_failure_rolls_back(failure: FailurePoint, primary_calls: &[&'static str]) {
    // Given
    let mut adapter = FakeUpdateAdapter::with_failure(Some(failure));

    // When
    let result = run_update_transaction(
        &mut adapter,
        UpdateTarget::Tag(TARGET_VERSION.to_owned()),
        "v0.1.0",
        HEALTH_TIMEOUT,
    );

    // Then
    assert!(matches!(result, Ok(TransactionOutcome::RolledBack { .. })));
    let mut expected_calls = vec!["stage", "backup"];
    expected_calls.extend_from_slice(primary_calls);
    expected_calls.push("rollback");
    if failure == FailurePoint::PreSwap || primary_calls.contains(&"start_task") {
        expected_calls.extend_from_slice(&["stop_task", "wait_until_stopped"]);
    }
    expected_calls.extend_from_slice(&[
        "restore_executable",
        "restart_original_task",
        "cleanup_backup",
        "cleanup_staging",
    ]);
    assert_eq!(adapter.state.calls, expected_calls);
}

#[test]
fn pre_swap_swap_post_swap_and_health_failures_restore_known_good_state() {
    assert_failure_rolls_back(FailurePoint::PreSwap, &["pre_swap"]);
    assert_failure_rolls_back(FailurePoint::Swap, &["pre_swap", "swap"]);
    assert_failure_rolls_back(FailurePoint::PostSwap, &["pre_swap", "swap", "post_swap"]);
    assert_failure_rolls_back(
        FailurePoint::Health,
        &["pre_swap", "swap", "post_swap", "start_task", "health"],
    );
}

#[test]
fn pre_swap_failure_with_original_task_running_stops_before_restoring_the_executable() {
    // Given
    let mut adapter = FakeUpdateAdapter::with_failure(Some(FailurePoint::PreSwap));

    // When
    let result = run_update_transaction(
        &mut adapter,
        UpdateTarget::Tag(TARGET_VERSION.to_owned()),
        "v0.1.0",
        HEALTH_TIMEOUT,
    );

    // Then
    assert!(matches!(result, Ok(TransactionOutcome::RolledBack { .. })));
    assert_eq!(adapter.count("start_task"), 0);
    assert_eq!(adapter.count("stop_task"), 1);
    let stop = adapter
        .state
        .calls
        .iter()
        .position(|call| *call == "stop_task");
    let stopped = adapter
        .state
        .calls
        .iter()
        .position(|call| *call == "wait_until_stopped");
    let restore = adapter
        .state
        .calls
        .iter()
        .position(|call| *call == "restore_executable");
    assert!(stop.is_some_and(|stop| {
        stopped.is_some_and(|stopped| {
            restore.is_some_and(|restore| stop < stopped && stopped < restore)
        })
    }));
}

#[test]
fn enabled_idle_task_starts_before_the_health_check() {
    // Given
    let mut adapter = FakeUpdateAdapter::with_failure(None);
    adapter.state.current_task = TaskSnapshot::new("<Task/>".to_owned(), true, false);
    adapter.state.task_running = false;

    // When
    let result = run_update_transaction(
        &mut adapter,
        UpdateTarget::Tag(TARGET_VERSION.to_owned()),
        "v0.1.0",
        HEALTH_TIMEOUT,
    );

    // Then
    assert!(matches!(result, Ok(TransactionOutcome::Installed { .. })));
    let start = adapter
        .state
        .calls
        .iter()
        .position(|call| *call == "start_task");
    let health = adapter
        .state
        .calls
        .iter()
        .position(|call| *call == "health");
    assert!(start.is_some_and(|start| health.is_some_and(|health| start < health)));
    assert!(adapter.state.task_running);
}

#[test]
fn enabled_idle_task_that_never_runs_rolls_back_the_attempted_start() {
    // Given
    let mut adapter = FakeUpdateAdapter::with_failure(Some(FailurePoint::TaskDoesNotReachRunning));
    let prior_task = TaskSnapshot::new("<Task><Idle/></Task>".to_owned(), true, false);
    let prior_executable = adapter.state.current_executable.clone();
    adapter.state.current_task = prior_task.clone();
    adapter.state.task_running = false;

    // When
    let result = run_update_transaction(
        &mut adapter,
        UpdateTarget::Tag(TARGET_VERSION.to_owned()),
        "v0.1.0",
        HEALTH_TIMEOUT,
    );

    // Then
    assert!(matches!(
        result,
        Ok(TransactionOutcome::RolledBack { cause })
            if matches!(*cause, wifimic_update::TransactionError::HealthCheck { .. })
    ));
    assert_eq!(adapter.count("start_task"), 1);
    assert_eq!(adapter.count("stop_task"), 0);
    assert_eq!(adapter.state.current_task, prior_task);
    assert_eq!(adapter.state.current_executable, prior_executable);
    assert!(!adapter.state.task_running);
}

#[test]
fn rollback_does_not_stop_an_original_task_before_the_update_starts_one() {
    // Given
    let mut adapter = FakeUpdateAdapter::with_failure(Some(FailurePoint::Swap));

    // When
    let result = run_update_transaction(
        &mut adapter,
        UpdateTarget::Tag(TARGET_VERSION.to_owned()),
        "v0.1.0",
        HEALTH_TIMEOUT,
    );

    // Then
    assert!(matches!(result, Ok(TransactionOutcome::RolledBack { .. })));
    assert_eq!(adapter.count("stop_task"), 0);
    assert_eq!(adapter.count("restart_original_task"), 1);
}

#[test]
fn rollback_verification_failure_retains_backup_and_cleans_staging() {
    // Given
    let mut adapter = FakeUpdateAdapter::with_failure(Some(FailurePoint::Swap));
    adapter.state.fail_restore = true;

    // When
    let result = run_update_transaction(
        &mut adapter,
        UpdateTarget::Tag(TARGET_VERSION.to_owned()),
        "v0.1.0",
        HEALTH_TIMEOUT,
    );

    // Then
    assert!(matches!(
        result,
        Ok(TransactionOutcome::RollbackVerificationFailed { .. })
    ));
    assert_eq!(adapter.count("cleanup_backup"), 0);
    assert_eq!(adapter.count("cleanup_staging"), 1);
}

#[test]
fn rollback_restores_exact_prior_task_snapshot_and_executable() {
    // Given
    let mut adapter = FakeUpdateAdapter::with_failure(Some(FailurePoint::PostSwap));
    let prior_task =
        TaskSnapshot::new("<Task><Custom>prior</Custom></Task>".to_owned(), true, true);
    let prior_executable = adapter.state.current_executable.clone();
    adapter.state.current_task = prior_task.clone();

    // When
    let result = run_update_transaction(
        &mut adapter,
        UpdateTarget::Tag(TARGET_VERSION.to_owned()),
        "v0.1.0",
        HEALTH_TIMEOUT,
    );

    // Then
    assert!(matches!(result, Ok(TransactionOutcome::RolledBack { .. })));
    assert_eq!(adapter.state.current_task, prior_task);
    assert_eq!(adapter.state.current_executable, prior_executable);
}
