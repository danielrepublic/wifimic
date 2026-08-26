use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::updater::{TaskSnapshot, UpdaterError, UpdaterOperations};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum FailurePoint {
    DisableTask,
    StopTask,
    Swap,
    RestoreTask,
    EnableTask,
    StartTask,
    Health,
    EndpointCheck,
}

#[derive(Debug)]
pub(super) struct FakeUpdaterState {
    pub(super) calls: Vec<&'static str>,
    pub(super) failure: Option<FailurePoint>,
    pub(super) primary_failure_triggered: bool,
    pub(super) fail_restore_executable: bool,
    pub(super) current_task: TaskSnapshot,
    pub(super) current_executable: Vec<u8>,
    pub(super) backup_executable: Option<Vec<u8>>,
}

#[derive(Debug)]
pub(super) struct FakeUpdaterOperations {
    pub(super) state: FakeUpdaterState,
}

impl FakeUpdaterOperations {
    pub(super) fn with_failure(failure: Option<FailurePoint>) -> Self {
        let current_task = TaskSnapshot::new(
            "<Task><Enabled>true</Enabled></Task>".to_owned(),
            true,
            true,
        );
        Self {
            state: FakeUpdaterState {
                calls: Vec::new(),
                failure,
                primary_failure_triggered: false,
                fail_restore_executable: false,
                current_task,
                current_executable: b"old-client".to_vec(),
                backup_executable: None,
            },
        }
    }

    fn fail_if(&mut self, point: FailurePoint) -> Result<(), UpdaterError> {
        if self.state.failure == Some(point) && !self.state.primary_failure_triggered {
            self.state.primary_failure_triggered = true;
            Err(UpdaterError::Operation {
                operation: match point {
                    FailurePoint::DisableTask => "disable_task",
                    FailurePoint::StopTask => "stop_task",
                    FailurePoint::Swap => "atomic_swap_executable",
                    FailurePoint::RestoreTask => "restore_task",
                    FailurePoint::EnableTask => "enable_task",
                    FailurePoint::StartTask => "start_task",
                    FailurePoint::Health => "wait_for_healthy",
                    FailurePoint::EndpointCheck => "check_render_endpoint_enumerable",
                },
            })
        } else {
            Ok(())
        }
    }

    pub(super) fn count(&self, operation: &'static str) -> usize {
        self.state
            .calls
            .iter()
            .filter(|called| **called == operation)
            .count()
    }
}

impl UpdaterOperations for FakeUpdaterOperations {
    fn resolve_latest_tag(&mut self) -> Result<String, UpdaterError> {
        self.state.calls.push("resolve_latest_tag");
        Ok("v0.2.0".to_owned())
    }

    fn download_and_verify(&mut self, _tag: &str) -> Result<PathBuf, UpdaterError> {
        self.state.calls.push("download_and_verify");
        Ok(PathBuf::from("staged"))
    }

    fn backup_current_executable(&mut self, _backup_path: &Path) -> Result<(), UpdaterError> {
        self.state.calls.push("backup_current_executable");
        self.state.backup_executable = Some(self.state.current_executable.clone());
        Ok(())
    }

    fn restore_executable(
        &mut self,
        _backup_path: &Path,
        _install_path: &Path,
    ) -> Result<(), UpdaterError> {
        self.state.calls.push("restore_executable");
        if self.state.fail_restore_executable {
            return Err(UpdaterError::Operation {
                operation: "restore_executable",
            });
        }
        let Some(backup) = &self.state.backup_executable else {
            return Err(UpdaterError::Operation {
                operation: "restore_executable",
            });
        };
        self.state.current_executable = backup.clone();
        Ok(())
    }

    fn get_task(&mut self) -> Result<TaskSnapshot, UpdaterError> {
        self.state.calls.push("get_task");
        Ok(self.state.current_task.clone())
    }

    fn disable_task(&mut self) -> Result<(), UpdaterError> {
        self.state.calls.push("disable_task");
        self.fail_if(FailurePoint::DisableTask)?;
        self.state.current_task = TaskSnapshot::new(
            self.state.current_task.xml().to_owned(),
            false,
            self.state.current_task.running(),
        );
        Ok(())
    }

    fn stop_task(&mut self) -> Result<(), UpdaterError> {
        self.state.calls.push("stop_task");
        self.fail_if(FailurePoint::StopTask)?;
        self.state.current_task = TaskSnapshot::new(
            self.state.current_task.xml().to_owned(),
            self.state.current_task.enabled(),
            false,
        );
        Ok(())
    }

    fn restore_task(&mut self, snapshot: &TaskSnapshot) -> Result<(), UpdaterError> {
        self.state.calls.push("restore_task");
        self.fail_if(FailurePoint::RestoreTask)?;
        self.state.current_task = snapshot.clone();
        Ok(())
    }

    fn enable_task(&mut self) -> Result<(), UpdaterError> {
        self.state.calls.push("enable_task");
        self.fail_if(FailurePoint::EnableTask)?;
        self.state.current_task = TaskSnapshot::new(
            self.state.current_task.xml().to_owned(),
            true,
            self.state.current_task.running(),
        );
        Ok(())
    }

    fn start_task(&mut self) -> Result<(), UpdaterError> {
        self.state.calls.push("start_task");
        self.fail_if(FailurePoint::StartTask)?;
        self.state.current_task = TaskSnapshot::new(
            self.state.current_task.xml().to_owned(),
            self.state.current_task.enabled(),
            true,
        );
        Ok(())
    }

    fn atomic_swap_executable(
        &mut self,
        _staged: &Path,
        _install_path: &Path,
    ) -> Result<(), UpdaterError> {
        self.state.calls.push("atomic_swap_executable");
        self.fail_if(FailurePoint::Swap)?;
        self.state.current_executable = b"new-client".to_vec();
        Ok(())
    }

    fn check_render_endpoint_enumerable(&mut self) -> Result<bool, UpdaterError> {
        self.state.calls.push("check_render_endpoint_enumerable");
        if self.fail_if(FailurePoint::EndpointCheck).is_err() {
            Ok(false)
        } else {
            Ok(true)
        }
    }

    fn wait_for_healthy(&mut self, _timeout: Duration) -> Result<bool, UpdaterError> {
        self.state.calls.push("wait_for_healthy");
        if self.fail_if(FailurePoint::Health).is_err() {
            Ok(false)
        } else {
            Ok(true)
        }
    }
}
