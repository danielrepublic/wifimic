use wifimic_update::UpdateTarget;

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
