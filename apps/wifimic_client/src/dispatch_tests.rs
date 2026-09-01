use wifimic_update::{CheckUpdateOutcome, render_check_update, UpdateTarget};

#[cfg(not(target_os = "windows"))]
use super::dispatch;
use super::requires_diagnostics;
use crate::cli;

#[test]
fn only_run_audio_requires_diagnostics_initialization() {
    // Given
    let cases = [
        (cli::Command::RunAudio, true),
        (cli::Command::Update, false),
        (cli::Command::Status, false),
        (cli::Command::Doctor, false),
        (
            cli::Command::Upgrade {
                target: UpdateTarget::Latest,
            },
            false,
        ),
        (
            cli::Command::InternalApplyUpgrade {
                tag: "v1.2.3".to_owned(),
            },
            false,
        ),
    ];

    // When / Then
    for (command, expected) in cases {
        assert_eq!(
            requires_diagnostics(&command),
            expected,
            "diagnostics requirement for {command:?}"
        );
    }
}

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
    // Given / When / Then: prove the rendered update check output identifies
    // the client binary as "wifimic_client" and not "wifimic_server", without
    // invoking dispatch( or discover_latest_tag (network).
    let outcome = CheckUpdateOutcome::UpdateAvailable {
        current: "v0.1.12".to_owned(),
        latest: "v0.2.0".to_owned(),
    };
    let result = Ok(outcome);

    let rendered = render_check_update(&result, "wifimic_client");
    assert!(rendered.contains("wifimic_client"));
    assert!(!rendered.contains("wifimic_server"));
}
