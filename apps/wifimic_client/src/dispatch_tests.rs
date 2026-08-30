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
