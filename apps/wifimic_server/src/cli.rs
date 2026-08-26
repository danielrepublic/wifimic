/// Selects the one-shot action or long-running service mode requested by a user.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Command {
    /// Starts the service, optionally enabling the existing diagnostic modes.
    Service {
        /// Prints the existing calibration responder message.
        calibrate: bool,
        /// Runs the existing latency diagnostic capture service.
        diagnose_latency: bool,
    },
    /// Prints the embedded build version.
    Version,
    /// Checks the latest public release without changing local state.
    Update,
    /// Downloads and installs a release with rollback protection.
    Upgrade { tag: Option<String> },
    /// Prints service version and systemd state.
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
    /// A subcommand received an unexpected trailing argument.
    #[error("unexpected argument {argument:?} after the command")]
    UnexpectedTrailing { argument: String },
    /// `upgrade --tag` was not followed by a tag value.
    #[error("upgrade --tag requires a vMAJOR.MINOR.PATCH value")]
    MissingTag,
}

/// Parses the fixed server CLI grammar and defaults to service mode.
///
/// # Errors
/// Returns [`CliParseError`] when an argument is unknown, duplicated, or missing
/// the value required by `upgrade --tag`.
pub(crate) fn parse_command<I>(arguments: I) -> Result<Command, CliParseError>
where
    I: IntoIterator<Item = String>,
{
    let mut arguments = arguments.into_iter();
    let _program = arguments.next();
    let Some(first) = arguments.next() else {
        return Ok(service_command(false, false));
    };

    match first.as_str() {
        "-v" | "--version" => finish_simple_command(Command::Version, arguments),
        "update" => finish_simple_command(Command::Update, arguments),
        "status" => finish_simple_command(Command::Status, arguments),
        "doctor" => finish_simple_command(Command::Doctor, arguments),
        "upgrade" => parse_upgrade(arguments),
        "--calibrate" | "--diagnose-latency" => parse_service_flags(first, arguments),
        _ => Err(CliParseError::Unrecognized { argument: first }),
    }
}

fn service_command(calibrate: bool, diagnose_latency: bool) -> Command {
    Command::Service {
        calibrate,
        diagnose_latency,
    }
}

fn parse_service_flags(
    first: String,
    arguments: impl Iterator<Item = String>,
) -> Result<Command, CliParseError> {
    let mut calibrate = first == "--calibrate";
    let mut diagnose_latency = first == "--diagnose-latency";
    for argument in arguments {
        match argument.as_str() {
            "--calibrate" if !calibrate => calibrate = true,
            "--diagnose-latency" if !diagnose_latency => diagnose_latency = true,
            _ => return Err(CliParseError::Unrecognized { argument }),
        }
    }
    Ok(service_command(calibrate, diagnose_latency))
}

fn parse_upgrade(mut arguments: impl Iterator<Item = String>) -> Result<Command, CliParseError> {
    let Some(argument) = arguments.next() else {
        return Ok(Command::Upgrade { tag: None });
    };
    if argument != "--tag" {
        return Err(CliParseError::Unrecognized { argument });
    }
    let Some(tag) = arguments.next() else {
        return Err(CliParseError::MissingTag);
    };
    if tag.starts_with('-') {
        return Err(CliParseError::MissingTag);
    }
    if let Some(argument) = arguments.next() {
        return Err(CliParseError::UnexpectedTrailing { argument });
    }
    Ok(Command::Upgrade { tag: Some(tag) })
}

fn finish_simple_command(
    command: Command,
    mut arguments: impl Iterator<Item = String>,
) -> Result<Command, CliParseError> {
    if let Some(argument) = arguments.next() {
        return Err(CliParseError::UnexpectedTrailing { argument });
    }
    Ok(command)
}

#[cfg(test)]
mod tests {
    use super::{parse_command, CliParseError, Command};

    fn parse(arguments: &[&str]) -> Result<Command, CliParseError> {
        parse_command(arguments.iter().map(|argument| (*argument).to_owned()))
    }

    #[test]
    fn parses_service_and_one_shot_commands() {
        // Given
        let cases = [
            (
                vec!["wifimic_server"],
                Command::Service {
                    calibrate: false,
                    diagnose_latency: false,
                },
            ),
            (
                vec!["wifimic_server", "--calibrate"],
                Command::Service {
                    calibrate: true,
                    diagnose_latency: false,
                },
            ),
            (
                vec!["wifimic_server", "--diagnose-latency"],
                Command::Service {
                    calibrate: false,
                    diagnose_latency: true,
                },
            ),
            (
                vec!["wifimic_server", "--calibrate", "--diagnose-latency"],
                Command::Service {
                    calibrate: true,
                    diagnose_latency: true,
                },
            ),
            (vec!["wifimic_server", "-v"], Command::Version),
            (vec!["wifimic_server", "--version"], Command::Version),
            (vec!["wifimic_server", "update"], Command::Update),
            (
                vec!["wifimic_server", "upgrade"],
                Command::Upgrade { tag: None },
            ),
            (
                vec!["wifimic_server", "upgrade", "--tag", "v0.2.0"],
                Command::Upgrade {
                    tag: Some("v0.2.0".to_owned()),
                },
            ),
            (vec!["wifimic_server", "status"], Command::Status),
            (vec!["wifimic_server", "doctor"], Command::Doctor),
        ];

        // When
        let results = cases.iter().map(|(arguments, _expected)| parse(arguments));

        // Then
        for ((_, expected), result) in cases.iter().zip(results) {
            assert_eq!(result.expect("valid CLI case"), *expected);
        }
    }

    #[test]
    fn rejects_missing_upgrade_tag_without_panicking() {
        // Given
        let arguments = ["wifimic_server", "upgrade", "--tag"];

        // When
        let result = parse(&arguments);

        // Then
        assert_eq!(result, Err(CliParseError::MissingTag));
    }

    #[test]
    fn rejects_unrecognized_arguments() {
        // Given
        let arguments = ["wifimic_server", "--surprise"];

        // When
        let result = parse(&arguments);

        // Then
        assert_eq!(
            result,
            Err(CliParseError::Unrecognized {
                argument: "--surprise".to_owned()
            })
        );
    }

    #[test]
    fn rejects_check_update_as_unrecognized() {
        // Given
        let arguments = ["wifimic_server", "check-update"];

        // When
        let result = parse(&arguments);

        // Then
        assert_eq!(
            result,
            Err(CliParseError::Unrecognized {
                argument: "check-update".to_owned()
            })
        );
    }
}
