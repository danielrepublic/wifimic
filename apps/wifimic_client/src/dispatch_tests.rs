use wifimic_update::check::{render_check_update, CheckUpdateOutcome};

#[cfg(not(target_os = "windows"))]
use super::dispatch;
#[cfg(not(target_os = "windows"))]
use crate::cli;

#[cfg(not(target_os = "windows"))]
#[test]
fn internal_apply_upgrade_is_rejected_outside_windows() {
    // Given
    let command = cli::Command::InternalApplyUpgrade {
        tag: "v1.2.3".to_owned(),
    };

    // When
    let result = dispatch(command);

    // Then
    let error = result.expect_err("internal upgrade is Windows-only");
    assert_eq!(
        error.to_string(),
        "--internal-apply-upgrade is Windows-only"
    );
}

#[test]
fn client_binary_is_wifimic_client_and_not_server() {
    // Given
    let outcome = CheckUpdateOutcome::UpdateAvailable {
        current: "v0.1.12".to_owned(),
        latest: "v0.2.0".to_owned(),
    };
    let result = Ok(outcome);

    // When
    let rendered = render_check_update(&result, "wifimic_client");

    // Then
    assert!(rendered.contains("wifimic_client upgrade"));
    assert!(!rendered.contains("wifimic_server"));
}
