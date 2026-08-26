use std::path::{Path, PathBuf};
use std::time::Duration;

use super::{
    ExecutableSnapshot, FirewallSnapshot, InstallerError, InstallerOperations, TaskSnapshot,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum FailurePoint {
    Swap,
    Register,
    Firewall,
    Enable,
    Health,
}

#[derive(Debug, Default)]
pub(super) struct FakeInstallerOperations {
    pub(super) calls: Vec<&'static str>,
    pub(super) failure: Option<FailurePoint>,
    pub(super) fail_restore: bool,
}

impl FakeInstallerOperations {
    pub(super) fn with_failure(failure: Option<FailurePoint>) -> Self {
        Self {
            failure,
            ..Self::default()
        }
    }
    pub(super) fn count(&self, name: &'static str) -> usize {
        self.calls.iter().filter(|call| **call == name).count()
    }
    fn fail_if(&self, point: FailurePoint, operation: &'static str) -> Result<(), InstallerError> {
        if self.failure == Some(point) {
            Err(InstallerError::Operation {
                operation,
                message: "fake failure".to_owned(),
            })
        } else {
            Ok(())
        }
    }
    fn snapshot() -> ExecutableSnapshot {
        ExecutableSnapshot {
            bytes: b"prior".to_vec(),
            sha256: "prior".to_owned(),
        }
    }
}

impl InstallerOperations for FakeInstallerOperations {
    fn check_preflight(&mut self) -> Result<(), InstallerError> {
        self.calls.push("preflight");
        Ok(())
    }
    fn get_current_task(&mut self) -> Result<Option<TaskSnapshot>, InstallerError> {
        self.calls.push("task");
        Ok(Some(TaskSnapshot {
            xml: "prior".to_owned(),
            enabled: true,
            running: true,
        }))
    }
    fn get_current_firewall(&mut self) -> Result<Option<FirewallSnapshot>, InstallerError> {
        self.calls.push("firewall-read");
        Ok(Some(FirewallSnapshot {
            name: "wifimic-client".to_owned(),
            display_name: "wifimic-client".to_owned(),
            remote_address: "192.168.0.210/32".to_owned(),
            enabled: true,
        }))
    }
    fn get_current_executable_hash(
        &mut self,
    ) -> Result<Option<ExecutableSnapshot>, InstallerError> {
        self.calls.push("executable");
        Ok(Some(Self::snapshot()))
    }
    fn stage_and_verify_download(&mut self, _source: &Path) -> Result<PathBuf, InstallerError> {
        self.calls.push("stage");
        Ok(PathBuf::from("staged.exe"))
    }
    fn register_task(&mut self, _xml: &str, _enabled: bool) -> Result<(), InstallerError> {
        self.calls.push("register");
        self.fail_if(FailurePoint::Register, "register")
    }
    fn set_firewall(&mut self) -> Result<(), InstallerError> {
        self.calls.push("firewall");
        self.fail_if(FailurePoint::Firewall, "firewall")
    }
    fn disable_stop_task(&mut self) -> Result<(), InstallerError> {
        self.calls.push("disable-stop");
        Ok(())
    }
    fn atomic_swap_executable(&mut self, _staged: &Path) -> Result<(), InstallerError> {
        self.calls.push("swap");
        self.fail_if(FailurePoint::Swap, "swap")
    }
    fn enable_start_task(&mut self) -> Result<(), InstallerError> {
        self.calls.push("enable");
        self.fail_if(FailurePoint::Enable, "enable")
    }
    fn wait_for_healthy(&mut self, _timeout: Duration) -> Result<(), InstallerError> {
        self.calls.push("health");
        self.fail_if(FailurePoint::Health, "health")
    }
    fn restore_task(&mut self, _snapshot: Option<&TaskSnapshot>) -> Result<(), InstallerError> {
        self.calls.push("restore-task");
        if self.fail_restore {
            Err(InstallerError::Operation {
                operation: "restore-task",
                message: "fake rollback failure".to_owned(),
            })
        } else {
            Ok(())
        }
    }
    fn restore_firewall(
        &mut self,
        _snapshot: Option<&FirewallSnapshot>,
    ) -> Result<(), InstallerError> {
        self.calls.push("restore-firewall");
        if self.fail_restore {
            Err(InstallerError::Operation {
                operation: "restore-firewall",
                message: "fake rollback failure".to_owned(),
            })
        } else {
            Ok(())
        }
    }
    fn restore_executable(
        &mut self,
        _snapshot: Option<&ExecutableSnapshot>,
    ) -> Result<(), InstallerError> {
        self.calls.push("restore-executable");
        if self.fail_restore {
            Err(InstallerError::Operation {
                operation: "restore-executable",
                message: "fake rollback failure".to_owned(),
            })
        } else {
            Ok(())
        }
    }
}
