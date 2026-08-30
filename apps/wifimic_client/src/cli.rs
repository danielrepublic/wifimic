/// Selects the one-shot action or default audio-streaming mode requested by a user.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Command {
    /// Runs the existing audio client behavior (default, `--calibrate`, `--diagnose-latency`).
    RunAudio,
    /// Checks the latest public release without changing local state.
    Update,
    /// Downloads and installs a release via the elevated handoff script.
    Upgrade {
        target: wifimic_update::UpdateTarget,
    },
    /// Prints client version and scheduled-task state.
    Status,
    /// Runs the one-shot host self-check.
    Doctor,
    /// Hidden entry point invoked by the elevated handoff script to run the
    /// update transaction with `WindowsUpgradeAdapter`. Never documented in
    /// README/docs or any `--help`-equivalent output.
    InternalApplyUpgrade { tag: String },
}

/// Reports malformed command-line input without terminating the process abruptly.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub(crate) enum CliParseError {
    /// An argument is not part of the fixed command grammar.
    #[error("unrecognized argument {argument:?}")]
    Unrecognized { argument: String },
    /// A subcommand received an unexpected trailing argument.
    #[error("unexpected argument {argument:?} after the command")]
    UnexpectedTrailing { argument: String },
}

/// Parses the fixed client CLI grammar and defaults to [`Command::RunAudio`].
///
/// Mirrors `wifimic_server`'s `parse_command` argv contract: `argv[0]` (the
/// executable name) is consumed and ignored. The existing `--calibrate` and
/// `--diagnose-latency` flags remain [`Command::RunAudio`] so `main`'s current
/// flag-specific handling is unaffected by this dispatch module.
///
/// # Errors
/// Returns [`CliParseError::Unrecognized`] when the first user argument does
/// not match the fixed grammar.
pub(crate) fn parse_command<I>(arguments: I) -> Result<Command, CliParseError>
where
    I: IntoIterator<Item = String>,
{
    let mut arguments = arguments.into_iter();
    let _program = arguments.next();
    let Some(first) = arguments.next() else {
        return Ok(Command::RunAudio);
    };

    match first.as_str() {
        "--calibrate" | "--diagnose-latency" => Ok(Command::RunAudio),
        "update" => Ok(Command::Update),
        "status" => Ok(Command::Status),
        "doctor" => Ok(Command::Doctor),
        "upgrade" => parse_upgrade(arguments),
        "--internal-apply-upgrade" => parse_internal_apply_upgrade(arguments),
        _ => Err(CliParseError::Unrecognized { argument: first }),
    }
}

/// Parses the `upgrade [latest|vX.Y.Z]` positional grammar.
///
/// Intentionally duplicates `wifimic_server`'s `parse_upgrade` (rather than
/// sharing a function across the two CLI crates) so each platform's CLI
/// binary keeps its own dependency-free grammar module; the two are kept in
/// grammar parity by contract and by mirrored test vectors, not by shared
/// code.
///
/// # Errors
/// Returns [`CliParseError::Unrecognized`] when the target argument is
/// neither `latest` nor a strict `vMAJOR.MINOR.PATCH` release tag. Returns
/// [`CliParseError::UnexpectedTrailing`] when a second argument follows the
/// target.
fn parse_upgrade(mut arguments: impl Iterator<Item = String>) -> Result<Command, CliParseError> {
    let Some(argument) = arguments.next() else {
        return Ok(Command::Upgrade {
            target: wifimic_update::UpdateTarget::Latest,
        });
    };
    let target = wifimic_update::parse_update_target(Some(&argument))
        .map_err(|_error| CliParseError::Unrecognized { argument })?;
    if let Some(argument) = arguments.next() {
        return Err(CliParseError::UnexpectedTrailing { argument });
    }
    Ok(Command::Upgrade { target })
}

/// Parses the hidden `--internal-apply-upgrade <tag>` entry point.
///
/// The tag is validated against [`wifimic_update::is_release_tag`] at this
/// parse boundary — the injection guard that lets todo 14's handoff-script
/// template substitution trust an already-validated tag rather than an
/// arbitrary string.
///
/// # Errors
/// Returns [`CliParseError::Unrecognized`] when the tag argument is missing
/// or malformed. Returns [`CliParseError::UnexpectedTrailing`] when a second
/// argument follows the tag.
fn parse_internal_apply_upgrade(
    mut arguments: impl Iterator<Item = String>,
) -> Result<Command, CliParseError> {
    let Some(tag) = arguments.next() else {
        return Err(CliParseError::Unrecognized {
            argument: "--internal-apply-upgrade".to_owned(),
        });
    };
    if !wifimic_update::is_release_tag(&tag) {
        return Err(CliParseError::Unrecognized { argument: tag });
    }
    if let Some(argument) = arguments.next() {
        return Err(CliParseError::UnexpectedTrailing { argument });
    }
    Ok(Command::InternalApplyUpgrade { tag })
}

#[cfg(test)]
mod tests {
    use wifimic_update::UpdateTarget;

    use super::{parse_command, CliParseError, Command};

    fn parse(arguments: &[&str]) -> Result<Command, CliParseError> {
        parse_command(arguments.iter().map(|argument| (*argument).to_owned()))
    }

    #[test]
    fn parses_default_and_flag_forms_as_run_audio() {
        // Given
        let cases = [
            vec!["wifimic_client"],
            vec!["wifimic_client", "--calibrate"],
            vec!["wifimic_client", "--diagnose-latency"],
        ];

        // When
        let results = cases.iter().map(|arguments| parse(arguments));

        // Then
        for result in results {
            assert_eq!(result.expect("valid CLI case"), Command::RunAudio);
        }
    }

    #[test]
    fn parses_update_status_and_doctor_commands() {
        // Given
        let cases = [
            (vec!["wifimic_client", "update"], Command::Update),
            (vec!["wifimic_client", "status"], Command::Status),
            (vec!["wifimic_client", "doctor"], Command::Doctor),
        ];

        // When
        let results = cases.iter().map(|(arguments, _expected)| parse(arguments));

        // Then
        for ((_, expected), result) in cases.iter().zip(results) {
            assert_eq!(result.expect("valid CLI case"), *expected);
        }
    }

    #[test]
    fn rejects_unrecognized_arguments() {
        // Given
        let arguments = ["wifimic_client", "surprise"];

        // When
        let result = parse(&arguments);

        // Then
        assert_eq!(
            result,
            Err(CliParseError::Unrecognized {
                argument: "surprise".to_owned()
            })
        );
    }

    #[test]
    fn parses_upgrade_with_no_argument_latest_and_a_tag() {
        // Given
        let cases = [
            (
                vec!["wifimic_client", "upgrade"],
                Command::Upgrade {
                    target: UpdateTarget::Latest,
                },
            ),
            (
                vec!["wifimic_client", "upgrade", "latest"],
                Command::Upgrade {
                    target: UpdateTarget::Latest,
                },
            ),
            (
                vec!["wifimic_client", "upgrade", "v1.2.3"],
                Command::Upgrade {
                    target: UpdateTarget::Tag("v1.2.3".to_owned()),
                },
            ),
        ];

        // When
        let results = cases.iter().map(|(arguments, _expected)| parse(arguments));

        // Then
        for ((_, expected), result) in cases.iter().zip(results) {
            assert_eq!(result.expect("valid CLI case"), *expected);
        }
    }

    #[test]
    fn rejects_upgrade_with_invalid_tag_as_unrecognized() {
        // Given
        let arguments = ["wifimic_client", "upgrade", "not-a-tag"];

        // When
        let result = parse(&arguments);

        // Then
        assert_eq!(
            result,
            Err(CliParseError::Unrecognized {
                argument: "not-a-tag".to_owned()
            })
        );
    }

    #[test]
    fn rejects_upgrade_with_trailing_argument_after_target() {
        // Given
        let arguments = ["wifimic_client", "upgrade", "v1.2.3", "extra"];

        // When
        let result = parse(&arguments);

        // Then
        assert_eq!(
            result,
            Err(CliParseError::UnexpectedTrailing {
                argument: "extra".to_owned()
            })
        );
    }

    #[test]
    fn parses_internal_apply_upgrade_with_a_valid_tag() {
        // Given
        let arguments = ["wifimic_client", "--internal-apply-upgrade", "v1.2.3"];

        // When
        let result = parse(&arguments);

        // Then
        assert_eq!(
            result.expect("valid CLI case"),
            Command::InternalApplyUpgrade {
                tag: "v1.2.3".to_owned()
            }
        );
    }

    #[test]
    fn rejects_internal_apply_upgrade_missing_the_tag_argument() {
        // Given
        let arguments = ["wifimic_client", "--internal-apply-upgrade"];

        // When
        let result = parse(&arguments);

        // Then
        assert_eq!(
            result,
            Err(CliParseError::Unrecognized {
                argument: "--internal-apply-upgrade".to_owned()
            })
        );
    }

    #[test]
    fn rejects_internal_apply_upgrade_with_an_unvalidated_tag_injection_guard() {
        // Given
        let arguments = [
            "wifimic_client",
            "--internal-apply-upgrade",
            "not-a-tag; rm -rf /",
        ];

        // When
        let result = parse(&arguments);

        // Then
        assert_eq!(
            result,
            Err(CliParseError::Unrecognized {
                argument: "not-a-tag; rm -rf /".to_owned()
            })
        );
    }

    #[test]
    fn rejects_internal_apply_upgrade_with_trailing_argument_after_tag() {
        // Given
        let arguments = [
            "wifimic_client",
            "--internal-apply-upgrade",
            "v1.2.3",
            "extra",
        ];

        // When
        let result = parse(&arguments);

        // Then
        assert_eq!(
            result,
            Err(CliParseError::UnexpectedTrailing {
                argument: "extra".to_owned()
            })
        );
    }
}
