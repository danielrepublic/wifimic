use std::ffi::OsStr;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crate::updater::{TaskSnapshot, UpdaterError, UpdaterOperations};

const TASK_PATH: &str = r"\wifimic\wifimic-client";
const CLIENT_INSTALL_PATH: &str = r"C:\Program Files\wifimic-client\wifimic_client.exe";
const HEALTH_POLL_INTERVAL: Duration = Duration::from_millis(250);

/// Executes the installed Windows client's update operations.
#[derive(Debug, Default)]
pub struct NativeUpdaterOperations;

/// Rejects every command-line argument beyond the process name.
pub fn validate_no_arguments(args: &[String]) -> Result<(), &'static str> {
    if args.len() > 1 {
        Err("wifimic_client_updater does not accept command-line arguments")
    } else {
        Ok(())
    }
}

#[cfg(test)]
#[test]
fn rejects_any_cli_argument_before_any_side_effect() {
    // Given
    let args = vec!["wifimic_client_updater".to_owned(), "--tag".to_owned()];

    // When
    let result = validate_no_arguments(&args);

    // Then
    assert_eq!(
        result,
        Err("wifimic_client_updater does not accept command-line arguments")
    );
}

impl UpdaterOperations for NativeUpdaterOperations {
    fn resolve_latest_tag(&mut self) -> Result<String, UpdaterError> {
        wifimic_update::discover_latest_tag().map_err(UpdaterError::from)
    }

    fn download_and_verify(&mut self, tag: &str) -> Result<PathBuf, UpdaterError> {
        crate::updater::download_and_verify_release(tag)
    }

    fn backup_current_executable(&mut self, backup_path: &Path) -> Result<(), UpdaterError> {
        fs::copy(client_install_path(), backup_path)
            .map(|_| ())
            .map_err(|error| UpdaterError::Backup {
                message: error.to_string(),
            })
    }

    fn restore_executable(
        &mut self,
        backup_path: &Path,
        install_path: &Path,
    ) -> Result<(), UpdaterError> {
        let temporary = sibling_path(install_path, ".wifimic_client.rollback");
        fs::copy(backup_path, &temporary).map_err(|error| UpdaterError::Restore {
            message: error.to_string(),
        })?;
        fs::rename(&temporary, install_path).map_err(|error| UpdaterError::Restore {
            message: error.to_string(),
        })
    }

    fn get_task(&mut self) -> Result<TaskSnapshot, UpdaterError> {
        let xml = run_schtasks("get_task_xml", ["/Query", "/TN", TASK_PATH, "/XML"])?;
        let xml = String::from_utf8(xml.stdout).map_err(|error| UpdaterError::Task {
            operation: "get_task_xml",
            message: error.to_string(),
        })?;
        let state = query_task_state()?;
        Ok(TaskSnapshot::new(xml, state.enabled, state.running))
    }

    fn disable_task(&mut self) -> Result<(), UpdaterError> {
        run_schtasks("disable_task", ["/Change", "/TN", TASK_PATH, "/DISABLE"]).map(|_| ())
    }

    fn stop_task(&mut self) -> Result<(), UpdaterError> {
        run_schtasks("stop_task", ["/End", "/TN", TASK_PATH]).map(|_| ())
    }

    fn restore_task(&mut self, snapshot: &TaskSnapshot) -> Result<(), UpdaterError> {
        let temporary = std::env::temp_dir().join(format!(
            "wifimic-client-task-{}-{}.xml",
            std::process::id(),
            timestamp()
        ));
        let result = (|| {
            {
                let mut file =
                    fs::File::create(&temporary).map_err(|error| UpdaterError::Task {
                        operation: "restore_task",
                        message: error.to_string(),
                    })?;
                file.write_all(&task_xml_bytes(snapshot.xml()))
                    .map_err(|error| UpdaterError::Task {
                        operation: "restore_task",
                        message: error.to_string(),
                    })?;
            }
            run_schtasks(
                "restore_task",
                vec![
                    OsStr::new("/Create").to_owned(),
                    OsStr::new("/TN").to_owned(),
                    OsStr::new(TASK_PATH).to_owned(),
                    OsStr::new("/XML").to_owned(),
                    temporary.as_os_str().to_owned(),
                    OsStr::new("/F").to_owned(),
                ],
            )?;
            let state_switch = if snapshot.enabled() {
                "/ENABLE"
            } else {
                "/DISABLE"
            };
            run_schtasks(
                "restore_task_state",
                ["/Change", "/TN", TASK_PATH, state_switch],
            )?;
            Ok(())
        })();
        let _ = fs::remove_file(&temporary);
        result
    }

    fn enable_task(&mut self) -> Result<(), UpdaterError> {
        run_schtasks("enable_task", ["/Change", "/TN", TASK_PATH, "/ENABLE"]).map(|_| ())
    }

    fn start_task(&mut self) -> Result<(), UpdaterError> {
        run_schtasks("start_task", ["/Run", "/TN", TASK_PATH]).map(|_| ())
    }

    fn atomic_swap_executable(
        &mut self,
        staged: &Path,
        install_path: &Path,
    ) -> Result<(), UpdaterError> {
        let temporary = sibling_path(install_path, ".wifimic_client.upgrade");
        fs::copy(staged, &temporary).map_err(|error| UpdaterError::Swap {
            message: error.to_string(),
        })?;
        fs::rename(&temporary, install_path).map_err(|error| UpdaterError::Swap {
            message: error.to_string(),
        })
    }

    fn check_render_endpoint_enumerable(&mut self) -> Result<bool, UpdaterError> {
        let endpoints = crate::render::enumerate_render_endpoints().map_err(|error| {
            UpdaterError::Endpoint {
                message: error.to_string(),
            }
        })?;
        Ok(endpoints
            .iter()
            .any(|endpoint| endpoint == crate::render::DEFAULT_RENDER_ENDPOINT))
    }

    fn wait_for_healthy(&mut self, timeout: Duration) -> Result<bool, UpdaterError> {
        let deadline = Instant::now() + timeout;
        loop {
            let state = query_task_state()?;
            if state.enabled && state.ready && self.check_render_endpoint_enumerable()? {
                return Ok(true);
            }
            if Instant::now() >= deadline {
                return Ok(false);
            }
            thread::sleep(HEALTH_POLL_INTERVAL);
        }
    }
}

#[derive(Debug, Default)]
struct TaskState {
    enabled: bool,
    running: bool,
    ready: bool,
}

fn run_schtasks<I, S>(operation: &'static str, args: I) -> Result<Output, UpdaterError>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let args = args
        .into_iter()
        .map(|argument| argument.as_ref().to_owned())
        .collect::<Vec<_>>();
    let output = Command::new("schtasks.exe")
        .args(&args)
        .output()
        .map_err(|error| UpdaterError::Task {
            operation,
            message: error.to_string(),
        })?;
    if output.status.success() {
        Ok(output)
    } else {
        Err(UpdaterError::Task {
            operation,
            message: command_output_message(&output),
        })
    }
}

fn command_output_message(output: &Output) -> String {
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
    if !stderr.is_empty() {
        return stderr;
    }
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    if !stdout.is_empty() {
        return stdout;
    }
    output.status.to_string()
}

fn query_task_state() -> Result<TaskState, UpdaterError> {
    let output = Command::new("powershell.exe")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            r#"$task = Get-ScheduledTask -TaskPath '\wifimic\' -TaskName 'wifimic-client' -ErrorAction Stop; Write-Output ([int]$task.Settings.Enabled); Write-Output ([string]$task.State)"#,
        ])
        .output()
        .map_err(|error| UpdaterError::Task {
            operation: "get_task_state",
            message: error.to_string(),
        })?;
    if !output.status.success() {
        return Err(UpdaterError::Task {
            operation: "get_task_state",
            message: command_output_message(&output),
        });
    }
    parse_task_state_output(&String::from_utf8_lossy(&output.stdout))
}

fn parse_task_state_output(output: &str) -> Result<TaskState, UpdaterError> {
    let mut values = output
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty());
    let enabled = match values.next() {
        Some("1") => true,
        Some("0") => false,
        Some(value) => {
            return Err(UpdaterError::Task {
                operation: "get_task_state",
                message: format!("unexpected enabled value: {value}"),
            });
        }
        None => {
            return Err(UpdaterError::Task {
                operation: "get_task_state",
                message: "missing enabled value".to_owned(),
            });
        }
    };
    let status = match values.next() {
        Some("Ready") => (false, true),
        Some("Running") => (true, true),
        Some(value) => {
            return Err(UpdaterError::Task {
                operation: "get_task_state",
                message: format!("unexpected task state: {value}"),
            });
        }
        None => {
            return Err(UpdaterError::Task {
                operation: "get_task_state",
                message: "missing task state".to_owned(),
            });
        }
    };
    Ok(TaskState {
        enabled,
        running: status.0,
        ready: status.1,
    })
}

fn task_xml_bytes(xml: &str) -> Vec<u8> {
    let mut bytes = vec![0xFF, 0xFE];
    for code_unit in xml.encode_utf16() {
        bytes.extend_from_slice(&code_unit.to_le_bytes());
    }
    bytes
}

fn client_install_path() -> &'static Path {
    Path::new(CLIENT_INSTALL_PATH)
}

fn sibling_path(path: &Path, prefix: &str) -> PathBuf {
    path.with_file_name(format!("{prefix}-{}-{}", std::process::id(), timestamp()))
}

fn timestamp() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_nanos())
}

#[cfg(test)]
mod tests {
    use super::{parse_task_state_output, task_xml_bytes};

    #[test]
    fn restore_task_serializes_declared_utf16_xml() {
        // Given
        let xml = "<?xml version=\"1.0\" encoding=\"UTF-16\"?>\r\n<Task/>";

        // When
        let bytes = task_xml_bytes(xml);

        // Then
        assert_eq!(&bytes[..2], &[0xFF, 0xFE]);
        let code_units = bytes[2..]
            .chunks_exact(2)
            .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
            .collect::<Vec<_>>();
        assert_eq!(
            String::from_utf16(&code_units).expect("UTF-16 bytes decode"),
            xml
        );
    }

    #[test]
    fn parses_locale_independent_task_state_output() {
        // Given
        let output = "1\r\nReady\r\n";

        // When
        let state = parse_task_state_output(output).expect("task state parses");

        // Then
        assert!(state.enabled);
        assert!(!state.running);
        assert!(state.ready);
    }
}
