use super::{check_update_exit_code, render_check_update, run_check_update, CheckUpdateOutcome};
use crate::UpdateError;

#[test]
fn check_update_reports_up_to_date_when_versions_match() {
    // Given
    let current = "v0.1.12";

    // When
    let result = run_check_update(current, || Ok("v0.1.12".to_owned()));

    // Then
    assert_eq!(
        result,
        Ok(CheckUpdateOutcome::UpToDate {
            current: "v0.1.12".to_owned(),
            latest: "v0.1.12".to_owned(),
        })
    );
    assert_eq!(
        render_check_update(&result, "wifimic_fixture"),
        "目前版本 v0.1.12 已是最新版本"
    );
    assert_eq!(check_update_exit_code(&result), 0);
}

#[test]
fn check_update_reports_available_release_when_latest_is_newer() {
    // Given
    let current = "v0.1.12";

    // When
    let result = run_check_update(current, || Ok("v0.2.0".to_owned()));

    // Then
    assert_eq!(
        result,
        Ok(CheckUpdateOutcome::UpdateAvailable {
            current: "v0.1.12".to_owned(),
            latest: "v0.2.0".to_owned(),
        })
    );
}

#[test]
fn check_update_reports_current_newer_when_latest_is_older() {
    // Given
    let current = "v0.2.0";

    // When
    let result = run_check_update(current, || Ok("v0.1.12".to_owned()));

    // Then
    assert_eq!(
        result,
        Ok(CheckUpdateOutcome::CurrentNewer {
            current: "v0.2.0".to_owned(),
            latest: "v0.1.12".to_owned(),
        })
    );
    assert_eq!(
        render_check_update(&result, "wifimic_fixture"),
        "目前版本 v0.2.0 比最新版本 v0.1.12 更新"
    );
}

#[test]
fn check_update_returns_network_failure_when_discovery_fails() {
    // Given
    let current = "v0.1.12";

    // When
    let result = run_check_update(current, || {
        Err(UpdateError::Network {
            message: "offline".to_owned(),
        })
    });

    // Then
    assert_eq!(
        result,
        Err(UpdateError::Network {
            message: "offline".to_owned(),
        })
    );
    assert_eq!(
        render_check_update(&result, "wifimic_fixture"),
        "更新檢查失敗：GitHub release request failed: offline"
    );
    assert_eq!(check_update_exit_code(&result), 1);
}

#[test]
fn check_update_returns_indeterminate_error_when_current_is_not_a_release_tag() {
    // Given
    let current = "v0.1.12-dev";

    // When
    let result = run_check_update(current, || Ok("v0.2.0".to_owned()));

    // Then
    assert_eq!(
        result,
        Err(UpdateError::IndeterminateVersion {
            current: "v0.1.12-dev".to_owned(),
        })
    );
}

#[test]
fn check_update_renders_caller_binary_with_upgrade_for_an_available_release() {
    // Given
    let result = run_check_update("v0.1.12", || Ok("v0.2.0".to_owned()));

    // When
    let rendered = render_check_update(&result, "wifimic_fixture");

    // Then
    assert_eq!(
        rendered,
        "有新版本可用：v0.1.12 → v0.2.0，執行 `wifimic_fixture upgrade` 進行更新"
    );
}
