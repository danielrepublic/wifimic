use wifimic_client::task_query::{TaskQuery, TaskQueryError, TaskState};

/// Reports a `status` command failure.
#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq)]
pub(crate) enum StatusError {
    /// The scheduled task's state could not be queried.
    #[error("could not query scheduled task state: {0}")]
    Task(#[from] TaskQueryError),
}

/// Contains the version and scheduled-task state displayed by `status`.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct StatusReport {
    /// The embedded client version.
    pub(crate) version: &'static str,
    /// The scheduled task's current state.
    pub(crate) task: TaskState,
}

impl StatusReport {
    fn render(&self) -> String {
        format!(
            "版本：{}；排程工作 enabled={} running={} ready={}",
            self.version, self.task.enabled, self.task.running, self.task.ready
        )
    }
}

/// Queries and formats the client's version and scheduled-task state.
///
/// # Errors
/// Returns [`StatusError`] when the scheduled task's state cannot be
/// queried — a query failure never falls back to a default/empty
/// [`TaskState`].
pub(crate) fn run_status<Q: TaskQuery>(
    queries: &Q,
    version: &'static str,
) -> Result<StatusReport, StatusError> {
    Ok(StatusReport {
        version,
        task: queries.state()?,
    })
}

/// Renders the one-line output for a status result.
pub(crate) fn render_status(result: &Result<StatusReport, StatusError>) -> String {
    match result {
        Ok(report) => report.render(),
        Err(error) => format!("狀態查詢失敗：{error}"),
    }
}

/// Returns the process exit code for a status result.
#[must_use]
pub(crate) fn status_exit_code(result: &Result<StatusReport, StatusError>) -> u8 {
    if result.is_ok() {
        0
    } else {
        1
    }
}

#[cfg(test)]
mod tests {
    use super::{render_status, run_status, status_exit_code, StatusError};
    use wifimic_client::task_query::{TaskQuery, TaskQueryError, TaskState};

    #[derive(Debug, Clone)]
    struct FakeQuery(Result<TaskState, TaskQueryError>);

    impl TaskQuery for FakeQuery {
        fn state(&self) -> Result<TaskState, TaskQueryError> {
            self.0.clone()
        }
    }

    #[test]
    fn reports_version_and_task_state_from_a_successful_query() {
        // Given
        let state = TaskState {
            enabled: true,
            running: false,
            ready: true,
        };
        let query = FakeQuery(Ok(state));

        // When
        let report = run_status(&query, "v0.1.12").expect("fake query succeeds");

        // Then
        assert_eq!(report.version, "v0.1.12");
        assert_eq!(report.task, state);
        assert_eq!(status_exit_code(&Ok(report)), 0);
    }

    #[test]
    fn renders_a_failure_message_and_nonzero_exit_when_the_query_fails() {
        // Given
        let query = FakeQuery(Err(TaskQueryError::Invoke {
            operation: "get_task_state",
            message: "schtasks.exe not found".to_owned(),
        }));

        // When
        let result = run_status(&query, "v0.1.12");

        // Then
        assert_eq!(status_exit_code(&result), 1);
        assert!(render_status(&result).contains("狀態查詢失敗"));
        assert!(matches!(result, Err(StatusError::Task(_))));
    }
}
