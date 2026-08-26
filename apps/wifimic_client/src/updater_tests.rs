use crate::updater::{run_update, TaskSnapshot, UpdaterOutcome};
use crate::updater_test_support::{FailurePoint, FakeUpdaterOperations};

const CURRENT_VERSION: &str = "v0.2.0";
const TARGET_VERSION: &str = "v0.2.0";

#[test]
fn already_up_to_date_returns_noop_and_makes_zero_calls() {
    // Given
    let mut operations = FakeUpdaterOperations::with_failure(None);

    // When
    let result = run_update(&mut operations, CURRENT_VERSION).expect("fake update succeeds");

    // Then
    assert_eq!(result, UpdaterOutcome::NoOp);
    assert_eq!(operations.count("backup_current_executable"), 0);
    assert_eq!(operations.count("get_task"), 0);
    assert_eq!(operations.count("disable_task"), 0);
    assert_eq!(operations.count("stop_task"), 0);
    assert_eq!(operations.count("atomic_swap_executable"), 0);
    assert_eq!(operations.count("check_render_endpoint_enumerable"), 0);
    assert_eq!(operations.count("wait_for_healthy"), 0);
}

#[test]
fn clean_update_records_expected_call_sequence_and_returns_installed() {
    // Given
    let mut operations = FakeUpdaterOperations::with_failure(None);

    // When
    let result = run_update(&mut operations, "v0.1.0").expect("fake update succeeds");

    // Then
    assert_eq!(
        result,
        UpdaterOutcome::Installed {
            tag: TARGET_VERSION.to_owned(),
        }
    );
    assert_eq!(
        operations.state.calls,
        vec![
            "resolve_latest_tag",
            "download_and_verify",
            "backup_current_executable",
            "get_task",
            "disable_task",
            "stop_task",
            "atomic_swap_executable",
            "restore_task",
            "enable_task",
            "start_task",
            "check_render_endpoint_enumerable",
            "wait_for_healthy",
        ]
    );
}

fn assert_failure_rolls_back(failure: FailurePoint, primary_calls: &[&'static str]) {
    // Given
    let mut operations = FakeUpdaterOperations::with_failure(Some(failure));

    // When
    let result = run_update(&mut operations, "v0.1.0").expect("rollback outcome is returned");

    // Then
    assert_eq!(result, UpdaterOutcome::RolledBack);
    let mut expected_calls = vec![
        "resolve_latest_tag",
        "download_and_verify",
        "backup_current_executable",
        "get_task",
    ];
    expected_calls.extend_from_slice(primary_calls);
    expected_calls.extend_from_slice(&["restore_executable", "restore_task", "start_task"]);
    assert_eq!(operations.state.calls, expected_calls);
}

#[test]
fn disable_task_failure_rolls_back_and_returns_rolled_back() {
    assert_failure_rolls_back(FailurePoint::DisableTask, &["disable_task"]);
}

#[test]
fn stop_task_failure_rolls_back_and_returns_rolled_back() {
    assert_failure_rolls_back(FailurePoint::StopTask, &["disable_task", "stop_task"]);
}

#[test]
fn swap_failure_rolls_back_and_returns_rolled_back() {
    assert_failure_rolls_back(
        FailurePoint::Swap,
        &["disable_task", "stop_task", "atomic_swap_executable"],
    );
}

#[test]
fn restore_task_failure_rolls_back_and_returns_rolled_back() {
    assert_failure_rolls_back(
        FailurePoint::RestoreTask,
        &[
            "disable_task",
            "stop_task",
            "atomic_swap_executable",
            "restore_task",
        ],
    );
}

#[test]
fn enable_task_failure_rolls_back_and_returns_rolled_back() {
    assert_failure_rolls_back(
        FailurePoint::EnableTask,
        &[
            "disable_task",
            "stop_task",
            "atomic_swap_executable",
            "restore_task",
            "enable_task",
        ],
    );
}

#[test]
fn start_task_failure_rolls_back_and_returns_rolled_back() {
    assert_failure_rolls_back(
        FailurePoint::StartTask,
        &[
            "disable_task",
            "stop_task",
            "atomic_swap_executable",
            "restore_task",
            "enable_task",
            "start_task",
        ],
    );
}

#[test]
fn health_failure_rolls_back_and_returns_rolled_back() {
    // Given
    let mut operations = FakeUpdaterOperations::with_failure(Some(FailurePoint::Health));

    // When
    let result = run_update(&mut operations, "v0.1.0").expect("rollback outcome is returned");

    // Then
    assert_eq!(result, UpdaterOutcome::RolledBack);
    assert_eq!(
        operations.state.calls,
        vec![
            "resolve_latest_tag",
            "download_and_verify",
            "backup_current_executable",
            "get_task",
            "disable_task",
            "stop_task",
            "atomic_swap_executable",
            "restore_task",
            "enable_task",
            "start_task",
            "check_render_endpoint_enumerable",
            "wait_for_healthy",
            "restore_executable",
            "restore_task",
            "start_task",
        ]
    );
}

#[test]
fn endpoint_check_failure_rolls_back_and_returns_rolled_back() {
    // Given
    let mut operations = FakeUpdaterOperations::with_failure(Some(FailurePoint::EndpointCheck));

    // When
    let result = run_update(&mut operations, "v0.1.0").expect("rollback outcome is returned");

    // Then
    assert_eq!(result, UpdaterOutcome::RolledBack);
    assert_eq!(
        operations.state.calls,
        vec![
            "resolve_latest_tag",
            "download_and_verify",
            "backup_current_executable",
            "get_task",
            "disable_task",
            "stop_task",
            "atomic_swap_executable",
            "restore_task",
            "enable_task",
            "start_task",
            "check_render_endpoint_enumerable",
            "restore_executable",
            "restore_task",
            "start_task",
        ]
    );
}

#[test]
fn rollback_verification_failure_returns_distinct_outcome() {
    // Given
    let mut operations = FakeUpdaterOperations::with_failure(Some(FailurePoint::Swap));
    operations.state.fail_restore_executable = true;

    // When
    let result = run_update(&mut operations, "v0.1.0").expect("rollback outcome is returned");

    // Then
    assert_eq!(result, UpdaterOutcome::RollbackVerificationFailed);
    assert_eq!(operations.count("restore_executable"), 1);
    assert_eq!(operations.count("restore_task"), 1);
    assert_eq!(operations.count("start_task"), 1);
}

#[test]
fn rollback_restores_exact_prior_task_snapshot() {
    // Given
    let mut operations = FakeUpdaterOperations::with_failure(Some(FailurePoint::RestoreTask));
    let prior_task =
        TaskSnapshot::new("<Task><Custom>prior</Custom></Task>".to_owned(), true, true);
    let prior_executable = operations.state.current_executable.clone();
    operations.state.current_task = prior_task.clone();

    // When
    let result = run_update(&mut operations, "v0.1.0").expect("rollback outcome is returned");

    // Then
    assert_eq!(result, UpdaterOutcome::RolledBack);
    assert_eq!(operations.state.current_task, prior_task);
    assert_eq!(operations.state.current_executable, prior_executable);
    assert_eq!(operations.count("start_task"), 1);
}

#[test]
fn rollback_does_not_restart_a_task_that_was_not_running() {
    // Given
    let mut operations = FakeUpdaterOperations::with_failure(Some(FailurePoint::Swap));
    let prior_task = TaskSnapshot::new(
        "<Task><Custom>stopped</Custom></Task>".to_owned(),
        false,
        false,
    );
    operations.state.current_task = prior_task.clone();

    // When
    let result = run_update(&mut operations, "v0.1.0").expect("rollback outcome is returned");

    // Then
    assert_eq!(result, UpdaterOutcome::RolledBack);
    assert_eq!(operations.state.current_task, prior_task);
    assert_eq!(operations.count("start_task"), 0);
}
