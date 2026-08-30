use std::process::{Command, Output};

use thiserror::Error;

/// Reports failures while querying the scheduled task's read-only state.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum TaskQueryError {
    /// The query process could not be started or exited unsuccessfully.
    #[error("could not invoke {operation}: {message}")]
    Invoke {
        operation: &'static str,
        message: String,
    },
    /// The query output did not match the expected shape.
    #[error("{operation} returned malformed output: {message}")]
    Malformed {
        operation: &'static str,
        message: String,
    },
}

/// Reports the scheduled task's read-only lifecycle state.
///
/// Three independent fields, not a 2-tuple: a task can be `ready` without
/// currently `running`, a distinction `wait_for_healthy` relies on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TaskState {
    /// Whether the task is enabled in Task Scheduler.
    pub enabled: bool,
    /// Whether the task is currently executing.
    pub running: bool,
    /// Whether the task is ready to run (idle or currently running).
    pub ready: bool,
}

/// Provides a read-only query for the installed scheduled task's state.
///
/// Implementations never mutate the task (no `/Change`, `/Create`,
/// `/Delete`) and never require an elevated session.
pub trait TaskQuery {
    /// Returns the task's current enabled/running/ready state.
    ///
    /// # Errors
    /// Returns [`TaskQueryError`] when the query process cannot be invoked
    /// or its output does not match the expected shape (for example the
    /// task does not exist).
    fn state(&self) -> Result<TaskState, TaskQueryError>;
}

/// Queries the real Windows Task Scheduler via `Get-ScheduledTask`.
#[derive(Debug, Default)]
pub struct NativeTaskQuery;

impl TaskQuery for NativeTaskQuery {
    fn state(&self) -> Result<TaskState, TaskQueryError> {
        let output = Command::new("powershell.exe")
            .args([
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                r#"$task = Get-ScheduledTask -TaskPath '\wifimic\' -TaskName 'wifimic-client' -ErrorAction Stop; Write-Output ([int]$task.Settings.Enabled); Write-Output ([string]$task.State)"#,
            ])
            .output()
            .map_err(|error| TaskQueryError::Invoke {
                operation: "get_task_state",
                message: error.to_string(),
            })?;
        if !output.status.success() {
            return Err(TaskQueryError::Invoke {
                operation: "get_task_state",
                message: command_output_message(&output),
            });
        }
        parse_task_state_output(&String::from_utf8_lossy(&output.stdout))
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

fn parse_task_state_output(output: &str) -> Result<TaskState, TaskQueryError> {
    let mut values = output
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty());
    let enabled = match values.next() {
        Some("1") => true,
        Some("0") => false,
        Some(value) => {
            return Err(TaskQueryError::Malformed {
                operation: "get_task_state",
                message: format!("unexpected enabled value: {value}"),
            });
        }
        None => {
            return Err(TaskQueryError::Malformed {
                operation: "get_task_state",
                message: "missing enabled value".to_owned(),
            });
        }
    };
    let (running, ready) = match values.next() {
        Some("Ready") => (false, true),
        Some("Running") => (true, true),
        Some(value) => {
            return Err(TaskQueryError::Malformed {
                operation: "get_task_state",
                message: format!("unexpected task state: {value}"),
            });
        }
        None => {
            return Err(TaskQueryError::Malformed {
                operation: "get_task_state",
                message: "missing task state".to_owned(),
            });
        }
    };
    Ok(TaskState {
        enabled,
        running,
        ready,
    })
}

#[cfg(test)]
mod tests {
    use super::{parse_task_state_output, TaskState};

    #[test]
    fn parses_a_ready_but_not_running_task_state() {
        // Given
        let output = "1\r\nReady\r\n";

        // When
        let state = parse_task_state_output(output).expect("task state parses");

        // Then
        assert_eq!(
            state,
            TaskState {
                enabled: true,
                running: false,
                ready: true,
            }
        );
    }

    #[test]
    fn parses_a_running_task_as_both_running_and_ready() {
        // Given
        let output = "0\r\nRunning\r\n";

        // When
        let state = parse_task_state_output(output).expect("task state parses");

        // Then
        assert_eq!(
            state,
            TaskState {
                enabled: false,
                running: true,
                ready: true,
            }
        );
    }

    #[test]
    fn rejects_an_unexpected_enabled_value() {
        // Given
        let output = "maybe\r\nReady\r\n";

        // When
        let result = parse_task_state_output(output);

        // Then
        assert!(matches!(result, Err(super::TaskQueryError::Malformed { .. })));
    }

    #[test]
    fn rejects_an_unexpected_task_state_value() {
        // Given
        let output = "1\r\nQueued\r\n";

        // When
        let result = parse_task_state_output(output);

        // Then
        assert!(matches!(result, Err(super::TaskQueryError::Malformed { .. })));
    }

    #[test]
    fn rejects_missing_output() {
        // Given
        let output = "";

        // When
        let result = parse_task_state_output(output);

        // Then
        assert!(matches!(result, Err(super::TaskQueryError::Malformed { .. })));
    }
}
