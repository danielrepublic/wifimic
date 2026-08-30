/// Selects the one-shot action or default audio-streaming mode requested by a user.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Command {
    /// Runs the existing audio client behavior (default, `--calibrate`, `--diagnose-latency`).
    RunAudio,
    /// Checks the latest public release without changing local state.
    Update,
    /// Prints client version and scheduled-task state.
    Status,
    /// Runs the one-shot host self-check.
    Doctor,
}

/// Reports malformed command-line input without terminating the process abruptly.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub(crate) enum CliParseError {
    /// An argument is not part of the fixed command grammar.
    #[error("unrecognized argument {argument:?}")]
    Unrecognized { argument: String },
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
        _ => Err(CliParseError::Unrecognized { argument: first }),
    }
}

#[cfg(test)]
mod tests {
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
}
