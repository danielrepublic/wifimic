use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const HANDOFF_SCRIPT_TEMPLATE: &str = include_str!("../assets/update-handoff.ps1.template");

/// Reports failure while preparing the elevated update handoff.
#[derive(Debug, thiserror::Error)]
pub(crate) enum HandoffError {
    /// The release tag is unsafe to pass to the handoff script.
    #[error("release tag {tag:?} is not vMAJOR.MINOR.PATCH")]
    InvalidTag { tag: String },
    /// The temporary handoff script could not be created.
    #[error("could not create update handoff script: {source}")]
    CreateScript {
        #[source]
        source: std::io::Error,
    },
    /// The handoff template could not be written to its temporary script.
    #[error("could not write update handoff script: {source}")]
    WriteScript {
        #[source]
        source: std::io::Error,
    },
    /// The installed executable path could not be resolved for the script.
    #[error("could not resolve installed client executable: {source}")]
    ClientExecutable {
        #[source]
        source: std::io::Error,
    },
    /// The temporary handoff script path could not be resolved.
    #[error("could not resolve update handoff script: {source}")]
    HandoffScript {
        #[source]
        source: std::io::Error,
    },
    /// Windows did not supply an absolute system directory.
    #[error("SystemRoot is missing or not an absolute path")]
    InvalidSystemRoot,
    /// A Windows argument cannot be represented safely for `ShellExecuteW`.
    #[error("Windows handoff argument contains a prohibited quote or control character")]
    UnsafeArgument,
    /// The UAC handoff launch could not be started.
    #[error("could not start elevated update handoff (ShellExecuteW code {code})")]
    Elevation { code: isize },
    /// The elevated launch is only available on Windows.
    #[cfg(not(target_os = "windows"))]
    #[error("elevated update handoff is only available on Windows")]
    WindowsOnly,
}

/// Supplies the two side effects after target resolution has approved an upgrade.
pub(crate) trait HandoffOperations {
    /// Discovers the latest release before any handoff state is written.
    fn discover_latest_tag(&self) -> Result<String, wifimic_update::UpdateError>;
    /// Writes the embedded handoff script for the already validated release tag.
    fn write_script(&self, tag: &str) -> Result<PathBuf, HandoffError>;
    /// Requests UAC elevation for the prepared handoff script.
    fn launch_script(&self, script: &Path, tag: &str) -> Result<(), HandoffError>;
}

/// Uses the installed client's native release discovery and Windows shell APIs.
#[derive(Debug, Default)]
pub(crate) struct NativeHandoffOperations;

impl HandoffOperations for NativeHandoffOperations {
    fn discover_latest_tag(&self) -> Result<String, wifimic_update::UpdateError> {
        wifimic_update::discover_latest_tag()
    }

    fn write_script(&self, tag: &str) -> Result<PathBuf, HandoffError> {
        write_handoff_script(tag)
    }

    fn launch_script(&self, script: &Path, tag: &str) -> Result<(), HandoffError> {
        launch_elevated_handoff(script, tag)
    }
}

/// Describes the observable result of an `upgrade` preflight.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum UpgradeOutcome {
    /// The selected target already matches the installed client.
    NoOp { current: String, latest: String },
    /// UAC accepted the handoff process for the selected release.
    HandoffLaunched { tag: String },
}

/// Resolves an update target before writing or elevating a handoff script.
///
/// # Errors
/// Returns target-resolution, temporary-script, or elevation-launch failures.
pub(crate) fn run_upgrade<O: HandoffOperations>(
    operations: &O,
    target: &wifimic_update::UpdateTarget,
    current_version: &str,
) -> Result<UpgradeOutcome, HandoffErrorOrUpdate> {
    match wifimic_update::resolve_action(target, current_version, || {
        operations.discover_latest_tag()
    })? {
        wifimic_update::ResolvedAction::NoOp { current, latest } => {
            Ok(UpgradeOutcome::NoOp { current, latest })
        }
        wifimic_update::ResolvedAction::Proceed { target_tag } => {
            let script = operations.write_script(&target_tag)?;
            operations.launch_script(&script, &target_tag)?;
            Ok(UpgradeOutcome::HandoffLaunched { tag: target_tag })
        }
    }
}

/// Reports an upgrade preflight or handoff side-effect failure.
#[derive(Debug, thiserror::Error)]
pub(crate) enum HandoffErrorOrUpdate {
    /// The requested target could not be resolved safely.
    #[error(transparent)]
    Update(#[from] wifimic_update::UpdateError),
    /// The handoff script could not be written or launched.
    #[error(transparent)]
    Handoff(#[from] HandoffError),
}

/// Writes the embedded handoff template into a new temporary PowerShell script.
///
/// # Errors
/// Returns [`HandoffError::InvalidTag`] before accessing the filesystem when
/// `tag` is not a strict stable release tag.
pub(crate) fn write_handoff_script(tag: &str) -> Result<PathBuf, HandoffError> {
    write_handoff_script_in(tag, &std::env::temp_dir())
}

fn write_handoff_script_in(tag: &str, directory: &Path) -> Result<PathBuf, HandoffError> {
    if !wifimic_update::is_release_tag(tag) {
        return Err(HandoffError::InvalidTag {
            tag: tag.to_owned(),
        });
    }

    let path = directory.join(format!(
        "wifimic-update-handoff-{}-{}.ps1",
        std::process::id(),
        timestamp()
    ));
    let mut script = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .map_err(|source| HandoffError::CreateScript { source })?;
    script
        .write_all(HANDOFF_SCRIPT_TEMPLATE.as_bytes())
        .map_err(|source| HandoffError::WriteScript { source })?;
    Ok(path)
}

fn timestamp() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_nanos())
}

/// Launches an already-written handoff script through UAC using absolute PowerShell.
///
/// # Errors
/// Returns [`HandoffError::InvalidTag`] before preparing Windows command-line
/// arguments when `tag` is not a strict stable release tag.
pub(crate) fn launch_elevated_handoff(script: &Path, tag: &str) -> Result<(), HandoffError> {
    if !wifimic_update::is_release_tag(tag) {
        return Err(HandoffError::InvalidTag {
            tag: tag.to_owned(),
        });
    }
    launch_elevated_handoff_platform(script, tag)
}

#[cfg(target_os = "windows")]
fn launch_elevated_handoff_platform(script: &Path, tag: &str) -> Result<(), HandoffError> {
    use std::ffi::OsStr;
    use windows::core::PCWSTR;
    use windows::Win32::UI::Shell::ShellExecuteW;
    use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

    let system_root = std::env::var_os("SystemRoot")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .ok_or(HandoffError::InvalidSystemRoot)?;
    let powershell = system_root
        .join("System32")
        .join("WindowsPowerShell")
        .join("v1.0")
        .join("powershell.exe");
    let client_executable = std::env::current_exe()
        .map_err(|source| HandoffError::ClientExecutable { source })?
        .canonicalize()
        .map_err(|source| HandoffError::ClientExecutable { source })?;
    let script = script
        .canonicalize()
        .map_err(|source| HandoffError::HandoffScript { source })?;
    let script = quote_windows_argument(script.as_os_str())?;
    let client_executable = quote_windows_argument(client_executable.as_os_str())?;
    let release_tag = quote_windows_argument(OsStr::new(tag))?;
    let parameters = format!(
        "-NoProfile -ExecutionPolicy Bypass -File {script} -ParentProcessId {} -ReleaseTag {release_tag} -ClientExecutable {client_executable}",
        std::process::id()
    );
    let verb = wide(OsStr::new("runas"));
    let powershell = wide(powershell.as_os_str());
    let parameters = wide(OsStr::new(&parameters));

    // SAFETY: [Category 8 — FFI boundary] Each `PCWSTR` points to a UTF-16,
    // NUL-terminated vector retained for this call; the `windows` binding owns
    // the ABI contract and `ShellExecuteW` does not retain these pointers.
    let result = unsafe {
        ShellExecuteW(
            None,
            PCWSTR::from_raw(verb.as_ptr()),
            PCWSTR::from_raw(powershell.as_ptr()),
            PCWSTR::from_raw(parameters.as_ptr()),
            PCWSTR::null(),
            SW_SHOWNORMAL,
        )
    };
    let code = result.0 as isize;
    if code <= 32 {
        return Err(HandoffError::Elevation { code });
    }
    Ok(())
}

#[cfg(target_os = "windows")]
fn quote_windows_argument(argument: &std::ffi::OsStr) -> Result<String, HandoffError> {
    let argument = argument.to_string_lossy();
    if argument
        .chars()
        .any(|character| matches!(character, '"' | '\0' | '\r' | '\n'))
    {
        return Err(HandoffError::UnsafeArgument);
    }
    Ok(format!("\"{argument}\""))
}

#[cfg(target_os = "windows")]
fn wide(value: &std::ffi::OsStr) -> Vec<u16> {
    use std::iter;
    use std::os::windows::ffi::OsStrExt;

    value.encode_wide().chain(iter::once(0)).collect()
}

#[cfg(not(target_os = "windows"))]
fn launch_elevated_handoff_platform(_script: &Path, _tag: &str) -> Result<(), HandoffError> {
    Err(HandoffError::WindowsOnly)
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::path::Path;

    use super::{
        run_upgrade, write_handoff_script_in, HandoffError, HandoffOperations, UpgradeOutcome,
        HANDOFF_SCRIPT_TEMPLATE,
    };
    use wifimic_update::{UpdateError, UpdateTarget};

    struct FakeHandoff {
        write_calls: Cell<u8>,
        launch_calls: Cell<u8>,
    }

    impl HandoffOperations for FakeHandoff {
        fn discover_latest_tag(&self) -> Result<String, UpdateError> {
            Ok("v1.2.3".to_owned())
        }

        fn write_script(&self, _tag: &str) -> Result<std::path::PathBuf, HandoffError> {
            self.write_calls.set(self.write_calls.get() + 1);
            Ok(std::path::PathBuf::from("handoff.ps1"))
        }

        fn launch_script(&self, _script: &Path, _tag: &str) -> Result<(), HandoffError> {
            self.launch_calls.set(self.launch_calls.get() + 1);
            Ok(())
        }
    }

    #[test]
    fn write_handoff_script_rejects_malformed_tag_before_creating_a_file() {
        // Given
        let malformed_tag = "v1.2.3; Remove-Item C:\\*";

        // When
        let result = write_handoff_script_in(malformed_tag, Path::new(r"Z:\\missing-directory"));

        // Then
        assert!(matches!(
            result,
            Err(HandoffError::InvalidTag { tag }) if tag == malformed_tag
        ));
    }

    #[test]
    fn handoff_template_declares_three_literal_parameters() {
        // Given
        let template = HANDOFF_SCRIPT_TEMPLATE;

        // When
        let parameter_count = template.matches("[Parameter(Mandatory = $true)]").count();

        // Then
        assert_eq!(parameter_count, 3);
        assert!(template.contains("$ParentProcessId"));
        assert!(template.contains("$ReleaseTag"));
        assert!(template.contains("$ClientExecutable"));
    }

    #[test]
    fn upgrade_no_op_never_writes_or_launches_a_handoff_script() {
        // Given
        let handoff = FakeHandoff {
            write_calls: Cell::new(0),
            launch_calls: Cell::new(0),
        };

        // When
        let result = run_upgrade(&handoff, &UpdateTarget::Latest, "v1.2.3")
            .expect("equal versions should not need elevation");

        // Then
        assert!(matches!(result, UpgradeOutcome::NoOp { .. }));
        assert_eq!(handoff.write_calls.get(), 0);
        assert_eq!(handoff.launch_calls.get(), 0);
    }
}
