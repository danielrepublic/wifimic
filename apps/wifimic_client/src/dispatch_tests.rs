use wifimic_update::UpdateTarget;

use super::{dispatch, requires_diagnostics, DispatchError};
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

#[test]
fn upgrade_is_a_deterministic_transitional_error() {
    // Given
    let command = cli::Command::Upgrade {
        target: UpdateTarget::Latest,
    };

    // When
    let result = dispatch(command);

    // Then
    let error = result.expect_err("upgrade must fail until todo 14 wires the handoff launcher");
    assert_eq!(
        error.downcast_ref::<DispatchError>(),
        Some(&DispatchError::UpgradeUnavailableUntilHandoff)
    );
}

#[test]
fn internal_apply_upgrade_is_a_deterministic_transitional_error() {
    // Given
    let command = cli::Command::InternalApplyUpgrade {
        tag: "v1.2.3".to_owned(),
    };

    // When
    let result = dispatch(command);

    // Then
    let error =
        result.expect_err("internal-apply-upgrade must fail until todo 15 wires the adapter");
    assert_eq!(
        error.downcast_ref::<DispatchError>(),
        Some(&DispatchError::InternalApplyUnavailableUntilAdapter)
    );
}
