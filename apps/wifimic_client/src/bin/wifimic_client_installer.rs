//! Thin, explicitly-confirmed host mutation entry point.

#![cfg(windows)]

use std::path::PathBuf;

use thiserror::Error;
use wifimic_client::installer::{
    run_install, run_upgrade, InstallerConfig, InstallerError, NativeInstallerOperations,
    RENDER_ENDPOINT,
};

const EXIT_OK: i32 = 0;
const EXIT_REFUSED: i32 = 10;
const EXIT_ARGUMENTS: i32 = 11;
const EXIT_MUTATION: i32 = 20;
const EXIT_ROLLBACK: i32 = 21;

#[derive(Debug, Error)]
enum CliError {
    #[error("usage: wifimic_client_installer install --client-executable <path> --render-endpoint <name> --accept-host-mutation | upgrade --client-executable <path> --accept-host-mutation")]
    Usage,
    #[error("host mutation requires --accept-host-mutation")]
    MissingAcceptance,
    #[error("unsupported render endpoint; this client requires {RENDER_ENDPOINT:?}")]
    UnsupportedEndpoint,
    #[error(transparent)]
    Installer(#[from] InstallerError),
}

#[derive(Debug, PartialEq, Eq)]
enum Command {
    Install {
        executable: PathBuf,
        endpoint: String,
    },
    Upgrade {
        executable: PathBuf,
    },
}

fn parse_command(mut args: impl Iterator<Item = String>) -> Result<Command, CliError> {
    let _program = args.next().ok_or(CliError::Usage)?;
    let subcommand = args.next().ok_or(CliError::Usage)?;
    let mut executable = None;
    let mut endpoint = None;
    let mut accepted = false;
    while let Some(argument) = args.next() {
        match argument.as_str() {
            "--client-executable" => {
                executable = Some(PathBuf::from(args.next().ok_or(CliError::Usage)?))
            }
            "--render-endpoint" => endpoint = Some(args.next().ok_or(CliError::Usage)?),
            "--accept-host-mutation" => accepted = true,
            _ => return Err(CliError::Usage),
        }
    }
    if !accepted {
        return Err(CliError::MissingAcceptance);
    }
    let executable = executable.ok_or(CliError::Usage)?;
    match subcommand.as_str() {
        "install" => {
            let endpoint = endpoint.ok_or(CliError::Usage)?;
            if endpoint != RENDER_ENDPOINT {
                return Err(CliError::UnsupportedEndpoint);
            }
            Ok(Command::Install {
                executable,
                endpoint,
            })
        }
        "upgrade" if endpoint.is_none() => Ok(Command::Upgrade { executable }),
        _ => Err(CliError::Usage),
    }
}

fn run(command: Command) -> Result<(), CliError> {
    let (config, install) = match command {
        Command::Install {
            executable,
            endpoint,
        } => (
            InstallerConfig {
                client_executable: executable,
                render_endpoint: endpoint,
            },
            true,
        ),
        Command::Upgrade { executable } => (
            InstallerConfig {
                client_executable: executable,
                render_endpoint: RENDER_ENDPOINT.to_owned(),
            },
            false,
        ),
    };
    let mut operations = NativeInstallerOperations;
    match install {
        true => run_install(&mut operations, &config)?,
        false => run_upgrade(&mut operations, &config)?,
    }
    Ok(())
}

fn main() {
    let result = std::env::args().collect::<Vec<_>>();
    let command = parse_command(result.into_iter());
    let result = command.and_then(run);
    match result {
        Ok(()) => std::process::exit(EXIT_OK),
        Err(error @ CliError::MissingAcceptance) => {
            eprintln!("{error}");
            std::process::exit(EXIT_REFUSED);
        }
        Err(CliError::Installer(InstallerError::Preflight(message))) => {
            eprintln!("host mutation refused: {message}");
            std::process::exit(EXIT_REFUSED);
        }
        Err(CliError::Installer(InstallerError::Rollback { .. })) => {
            eprintln!("installer rollback failed");
            std::process::exit(EXIT_ROLLBACK);
        }
        Err(error @ CliError::Usage) | Err(error @ CliError::UnsupportedEndpoint) => {
            eprintln!("{error}");
            std::process::exit(EXIT_ARGUMENTS);
        }
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(EXIT_MUTATION);
        }
    }
}
