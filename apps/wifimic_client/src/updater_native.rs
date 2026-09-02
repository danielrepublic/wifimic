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
const STRAY_PROCESS_TIMEOUT: Duration = Duration::from_secs(10);

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
        // Task Scheduler's `running` flag only reflects the instance it
        // launched and tracks. A client started outside the task (for
        // example from Explorer) still holds a lock on the install-path
        // executable while being invisible to `snapshot.task.running()`,
        // which previously let `swap` proceed straight into an Access
        // Denied failure and an automatic rollback (observed live).
        // Detect and terminate any such stray process directly instead of
        // trusting the scheduled task's state alone.
        terminate_stray_client_processes().map_err(pre_swap_error)?;
        Ok(())
    }

    fn swap(&mut self, staged: &Path, _snapshot: &Self::Snapshot) -> Result<(), TransactionError> {
        atomic_swap(&staged.join(CLIENT_EXECUTABLE_NAME), client_install_path()).map_err(swap_error)
    }

    fn post_swap(&mut self, snapshot: &Self::Snapshot) -> Result<(), TransactionError> {
        restore_task(&snapshot.task.with_logon_startup_delay()).map_err(post_swap_error)?;
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

/// Terminates any running `wifimic_client.exe` process other than this one.
///
/// Task Scheduler only tracks the instance it launched, so a process started
/// outside the task (double-clicked, launched from the tray, or otherwise
/// spawned under Explorer) can still hold a lock on
/// [`CLIENT_INSTALL_PATH`] while being reported as not running. Polling
/// after each termination attempt because a process can take a moment to
/// release its file handle after it stops running.
fn terminate_stray_client_processes() -> Result<(), String> {
    let deadline = Instant::now() + STRAY_PROCESS_TIMEOUT;
    loop {
        let strays = list_client_process_ids()?;
        if strays.is_empty() {
            return Ok(());
        }
        for pid in &strays {
            // Best-effort: the process may have already exited between the
            // list and this termination attempt.
            let _ = terminate_process(*pid);
        }
        if Instant::now() >= deadline {
            return Err(format!(
                "could not terminate stray {CLIENT_EXECUTABLE_NAME} process(es) holding the install path: {strays:?}"
            ));
        }
        std::thread::sleep(HEALTH_POLL_INTERVAL);
    }
}

fn list_client_process_ids() -> Result<Vec<u32>, String> {
    let output = Command::new("powershell.exe")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            &format!(
                "(Get-CimInstance Win32_Process -Filter \"Name='{CLIENT_EXECUTABLE_NAME}'\").ProcessId"
            ),
        ])
        .output()
        .map_err(|error| format!("list_client_processes: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "list_client_processes: {}",
            command_output_message(&output)
        ));
    }
    Ok(parse_process_ids(
        &String::from_utf8_lossy(&output.stdout),
        std::process::id(),
    ))
}

fn parse_process_ids(output: &str, exclude_pid: u32) -> Vec<u32> {
    output
        .lines()
        .filter_map(|line| line.trim().parse::<u32>().ok())
        .filter(|pid| *pid != exclude_pid)
        .collect()
}

fn terminate_process(pid: u32) -> Result<(), String> {
    let output = Command::new("powershell.exe")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            &format!("Stop-Process -Id {pid} -Force -ErrorAction Stop"),
        ])
        .output()
        .map_err(|error| format!("terminate_process({pid}): {error}"))?;
    if output.status.success() {
        Ok(())
    } else {
        Err(format!(
            "terminate_process({pid}): {}",
            command_output_message(&output)
        ))
    }
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
        {
            let mut file = fs::File::create(&temporary).map_err(|error| error.to_string())?;
            file.write_all(&task_xml_bytes(snapshot.xml()))
                .map_err(|error| error.to_string())?;
            // The write handle must close before schtasks.exe opens the same
            // path, or Windows can reject the read with "the process cannot
            // access the file because it is being used by another process"
            // (observed live: this is a real Windows file-sharing race that
            // fake-operations unit tests cannot reproduce).
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

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use super::{parse_process_ids, stop_task_if_running, task_is_healthy};
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

    #[test]
    fn parses_no_process_ids_from_empty_output() {
        // Given
        let output = "";

        // When
        let ids = parse_process_ids(output, 1234);

        // Then
        assert_eq!(ids, Vec::<u32>::new());
    }

    #[test]
    fn parses_multiple_process_ids_from_powershell_output() {
        // Given
        let output = "4242\r\n9001\r\n";

        // When
        let ids = parse_process_ids(output, 1234);

        // Then
        assert_eq!(ids, vec![4242, 9001]);
    }

    #[test]
    fn excludes_the_current_process_id() {
        // Given
        let output = "4242\r\n1234\r\n9001\r\n";

        // When
        let ids = parse_process_ids(output, 1234);

        // Then
        assert_eq!(ids, vec![4242, 9001]);
    }

    #[test]
    fn ignores_blank_and_non_numeric_lines() {
        // Given
        let output = "\r\n4242\r\n\r\n";

        // When
        let ids = parse_process_ids(output, 1234);

        // Then
        assert_eq!(ids, vec![4242]);
    }
}
