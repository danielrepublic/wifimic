use std::ffi::OsStr;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use wifimic_update::{RollbackOutcome, TransactionError, UpdateAdapter, UpdateError};

use crate::task_query::{
    command_output_message, wait_until_stopped, NativeTaskQuery, TaskQuery, TaskState,
};
use crate::updater::{task_xml_bytes, TaskSnapshot, CLIENT_EXECUTABLE_NAME, HEALTH_TIMEOUT};

const TASK_PATH: &str = r"\wifimic\wifimic-client";
pub(crate) const CLIENT_INSTALL_PATH: &str = r"C:\Program Files\wifimic-client\wifimic_client.exe";
const HEALTH_POLL_INTERVAL: Duration = Duration::from_millis(250);

/// Supplies Windows-specific mechanics to the shared update transaction.
#[derive(Debug, Default)]
pub struct WindowsUpgradeAdapter {
    snapshot_was_enabled: bool,
}

/// Captures the executable backup and scheduled-task state before replacement.
#[derive(Debug)]
pub struct WindowsUpdateSnapshot {
    backup_path: PathBuf,
    task: TaskSnapshot,
}

impl UpdateAdapter for WindowsUpgradeAdapter {
    type Snapshot = WindowsUpdateSnapshot;

    fn discover_latest_tag(&mut self) -> Result<String, UpdateError> {
        wifimic_update::discover_latest_tag()
    }

    fn stage(&mut self, tag: &str) -> Result<PathBuf, TransactionError> {
        crate::updater_archive::download_and_verify_release(tag)
    }

    fn backup(&mut self, _staged: &Path) -> Result<Self::Snapshot, TransactionError> {
        self.snapshot_was_enabled = false;
        let backup_path = backup_path();
        if let Err(error) = fs::copy(client_install_path(), &backup_path) {
            let _ = fs::remove_file(&backup_path);
            return Err(backup_error(error.to_string()));
        }
        let task = match task_snapshot() {
            Ok(task) => task,
            Err(error) => {
                let _ = fs::remove_file(&backup_path);
                return Err(error);
            }
        };
        self.snapshot_was_enabled = task.enabled();
        Ok(WindowsUpdateSnapshot { backup_path, task })
    }

    fn pre_swap(&mut self, snapshot: &Self::Snapshot) -> Result<(), TransactionError> {
        disable_task().map_err(pre_swap_error)?;
        if snapshot.task.running() {
            stop_task().map_err(pre_swap_error)?;
            let stopped = wait_until_stopped(&NativeTaskQuery, HEALTH_TIMEOUT)
                .map_err(|error| pre_swap_error(error.to_string()))?;
            if !stopped {
                return Err(pre_swap_error("scheduled task did not stop before timeout"));
            }
        }
        Ok(())
    }

    fn swap(&mut self, staged: &Path, _snapshot: &Self::Snapshot) -> Result<(), TransactionError> {
        atomic_swap(&staged.join(CLIENT_EXECUTABLE_NAME), client_install_path()).map_err(swap_error)
    }

    fn post_swap(&mut self, snapshot: &Self::Snapshot) -> Result<(), TransactionError> {
        restore_task(&snapshot.task).map_err(post_swap_error)?;
        if snapshot.task.enabled() {
            start_task().map_err(post_swap_error)?;
        }
        Ok(())
    }

    fn health_check(&mut self, timeout: Duration) -> Result<bool, TransactionError> {
        if self.snapshot_was_enabled {
            wait_for_healthy(timeout).map_err(health_query_error)
        } else {
            check_render_endpoint_enumerable().map_err(health_query_error)
        }
    }

    fn rollback(&mut self, snapshot: &Self::Snapshot) -> RollbackOutcome {
        let stopped = stop_task_if_running(&NativeTaskQuery, stop_task);
        let executable_restored =
            restore_executable(&snapshot.backup_path, client_install_path()).is_ok();
        let task_restored = restore_task(&snapshot.task).is_ok();
        let task_restarted = !snapshot.task.running() || start_task().is_ok();
        if stopped && executable_restored && task_restored && task_restarted {
            RollbackOutcome::Verified
        } else {
            RollbackOutcome::VerificationFailed
        }
    }

    fn cleanup_backup(&mut self, snapshot: &Self::Snapshot) {
        let _ = fs::remove_file(&snapshot.backup_path);
    }
}

fn stop_task_if_running<Q, F>(query: &Q, stop: F) -> bool
where
    Q: TaskQuery,
    F: FnOnce() -> Result<(), String>,
{
    let Ok(state) = query.state() else {
        return false;
    };
    if !state.running {
        return true;
    }
    stop().is_ok() && wait_until_stopped(query, HEALTH_TIMEOUT).is_ok_and(|value| value)
}

fn task_snapshot() -> Result<TaskSnapshot, TransactionError> {
    let xml = run_schtasks("get_task_xml", ["/Query", "/TN", TASK_PATH, "/XML"])
        .and_then(|output| String::from_utf8(output.stdout).map_err(|error| error.to_string()))
        .map_err(backup_error)?;
    let state = NativeTaskQuery
        .state()
        .map_err(|error| backup_error(error.to_string()))?;
    Ok(TaskSnapshot::new(xml, state.enabled, state.running))
}

fn disable_task() -> Result<(), String> {
    run_schtasks("disable_task", ["/Change", "/TN", TASK_PATH, "/DISABLE"]).map(|_| ())
}

fn stop_task() -> Result<(), String> {
    run_schtasks("stop_task", ["/End", "/TN", TASK_PATH]).map(|_| ())
}

fn start_task() -> Result<(), String> {
    run_schtasks("start_task", ["/Run", "/TN", TASK_PATH]).map(|_| ())
}

fn restore_task(snapshot: &TaskSnapshot) -> Result<(), String> {
    let temporary = std::env::temp_dir().join(format!(
        "wifimic-client-task-{}-{}.xml",
        std::process::id(),
        timestamp()
    ));
    let result = (|| {
        let mut file = fs::File::create(&temporary).map_err(|error| error.to_string())?;
        file.write_all(&task_xml_bytes(snapshot.xml()))
            .map_err(|error| error.to_string())?;
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

fn atomic_swap(staged: &Path, install_path: &Path) -> Result<(), String> {
    copy_then_rename(staged, install_path, ".wifimic_client.upgrade")
}

fn restore_executable(backup_path: &Path, install_path: &Path) -> Result<(), String> {
    copy_then_rename(backup_path, install_path, ".wifimic_client.rollback")
}

fn copy_then_rename(source: &Path, destination: &Path, prefix: &str) -> Result<(), String> {
    let temporary = sibling_path(destination, prefix);
    let result = (|| {
        fs::copy(source, &temporary).map_err(|error| error.to_string())?;
        fs::rename(&temporary, destination).map_err(|error| error.to_string())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

fn check_render_endpoint_enumerable() -> Result<bool, String> {
    let endpoints =
        crate::render::enumerate_render_endpoints().map_err(|error| error.to_string())?;
    Ok(endpoints
        .iter()
        .any(|endpoint| endpoint == crate::render::DEFAULT_RENDER_ENDPOINT))
}

fn wait_for_healthy(timeout: Duration) -> Result<bool, String> {
    let deadline = Instant::now() + timeout;
    loop {
        let state = NativeTaskQuery.state().map_err(|error| error.to_string())?;
        if task_is_healthy(state) && check_render_endpoint_enumerable()? {
            return Ok(true);
        }
        if Instant::now() >= deadline {
            return Ok(false);
        }
        std::thread::sleep(HEALTH_POLL_INTERVAL);
    }
}

const fn task_is_healthy(state: TaskState) -> bool {
    state.enabled && state.running
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use super::{stop_task_if_running, task_is_healthy};
    use crate::task_query::{TaskQuery, TaskQueryError, TaskState};

    struct RunningThenStoppedTaskQuery {
        calls: Cell<u8>,
    }

    impl TaskQuery for RunningThenStoppedTaskQuery {
        fn state(&self) -> Result<TaskState, TaskQueryError> {
            let calls = self.calls.get();
            self.calls.set(calls + 1);
            Ok(TaskState {
                enabled: false,
                running: calls == 0,
                ready: calls == 0,
            })
        }
    }

    #[test]
    fn ready_task_is_not_healthy_until_it_is_running() {
        // Given
        let state = TaskState {
            enabled: true,
            running: false,
            ready: true,
        };

        // When
        let healthy = task_is_healthy(state);

        // Then
        assert!(!healthy);
    }

    #[test]
    fn running_task_is_stopped_and_confirmed_before_rollback_restore() {
        // Given
        let query = RunningThenStoppedTaskQuery {
            calls: Cell::new(0),
        };
        let stop_called = Cell::new(false);

        // When
        let stopped = stop_task_if_running(&query, || {
            stop_called.set(true);
            Ok(())
        });

        // Then
        assert!(stopped);
        assert!(stop_called.get());
        assert_eq!(query.calls.get(), 2);
    }
}

fn run_schtasks<I, S>(operation: &'static str, args: I) -> Result<Output, String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let output = Command::new("schtasks.exe")
        .args(args)
        .output()
        .map_err(|error| format!("{operation}: {error}"))?;
    if output.status.success() {
        Ok(output)
    } else {
        Err(format!("{operation}: {}", command_output_message(&output)))
    }
}

fn client_install_path() -> &'static Path {
    Path::new(CLIENT_INSTALL_PATH)
}

fn backup_path() -> PathBuf {
    std::env::temp_dir().join(format!(
        "wifimic_client.backup.{}-{}",
        std::process::id(),
        timestamp()
    ))
}

fn sibling_path(path: &Path, prefix: &str) -> PathBuf {
    path.with_file_name(format!("{prefix}-{}-{}", std::process::id(), timestamp()))
}

fn timestamp() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_nanos())
}

fn backup_error(message: impl Into<String>) -> TransactionError {
    TransactionError::Backup {
        message: message.into(),
    }
}

fn pre_swap_error(message: impl Into<String>) -> TransactionError {
    TransactionError::PreSwap {
        message: message.into(),
    }
}

fn swap_error(message: impl Into<String>) -> TransactionError {
    TransactionError::Swap {
        message: message.into(),
    }
}

fn post_swap_error(message: impl Into<String>) -> TransactionError {
    TransactionError::PostSwap {
        message: message.into(),
    }
}

fn health_query_error(message: impl Into<String>) -> TransactionError {
    TransactionError::HealthQuery {
        message: message.into(),
    }
}
