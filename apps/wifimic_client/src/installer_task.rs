//! Direct Task Scheduler COM access for the Windows installer.

#![cfg(windows)]

use std::thread;
use std::time::{Duration, Instant};

use windows::core::BSTR;
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED,
};
use windows::Win32::System::TaskScheduler::{
    CLSID_CTaskScheduler, IRegisteredTask, ITaskService, TASK_CREATE_OR_UPDATE,
    TASK_LOGON_INTERACTIVE_TOKEN, TASK_STATE_READY, TASK_STATE_RUNNING, TASK_STATE_UNKNOWN,
};
use windows::Win32::System::Variant::VARIANT;

use crate::installer::{InstallerError, TaskSnapshot};

struct ComApartment;

impl ComApartment {
    fn initialize() -> Result<Self, InstallerError> {
        // SAFETY: This thread owns the COM apartment for the duration of the guard.
        unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) }
            .ok()
            .map_err(|error| InstallerError::Operation {
                operation: "task-com",
                message: error.to_string(),
            })?;
        Ok(Self)
    }
}

impl Drop for ComApartment {
    fn drop(&mut self) {
        // SAFETY: CoInitializeEx succeeded for this guard on this thread.
        unsafe { CoUninitialize() };
    }
}

fn service() -> Result<ITaskService, InstallerError> {
    // SAFETY: The COM apartment is initialized by each caller before creating this object.
    unsafe { CoCreateInstance(&CLSID_CTaskScheduler, None, CLSCTX_INPROC_SERVER) }.map_err(
        |error| InstallerError::Operation {
            operation: "task-service",
            message: error.to_string(),
        },
    )
}

fn with_task<T>(
    callback: impl FnOnce(&IRegisteredTask) -> Result<T, InstallerError>,
) -> Result<T, InstallerError> {
    let _com = ComApartment::initialize()?;
    let service = service()?;
    let root = BSTR::from(r"\wifimic\");
    // SAFETY: `root` is a valid NUL-terminated BSTR owned for this call.
    let folder =
        unsafe { service.GetFolder(&root) }.map_err(|error| InstallerError::Operation {
            operation: "task-root",
            message: error.to_string(),
        })?;
    let path = BSTR::from("wifimic-client");
    // SAFETY: The path is a fixed canonical task path and the folder remains alive for this call.
    let registered =
        unsafe { folder.GetTask(&path) }.map_err(|error| InstallerError::Operation {
            operation: "task-read",
            message: error.to_string(),
        })?;
    callback(&registered)
}

/// Reads the canonical task and returns `None` when it is not registered.
pub fn snapshot() -> Result<Option<TaskSnapshot>, InstallerError> {
    match with_task(|registered| {
        // SAFETY: `registered` is a live COM interface returned by Task Scheduler.
        let xml = unsafe { registered.Xml() }.map_err(|error| InstallerError::Operation {
            operation: "task-xml",
            message: error.to_string(),
        })?;
        // SAFETY: `registered` remains valid while querying these scalar properties.
        let enabled = unsafe { registered.Enabled() }
            .map_err(|error| InstallerError::Operation {
                operation: "task-enabled",
                message: error.to_string(),
            })?
            .as_bool();
        // SAFETY: `registered` remains valid while querying its state.
        let state = unsafe { registered.State() }.map_err(|error| InstallerError::Operation {
            operation: "task-state",
            message: error.to_string(),
        })?;
        Ok(TaskSnapshot {
            xml: xml.to_string(),
            enabled,
            running: state == TASK_STATE_RUNNING,
        })
    }) {
        Ok(snapshot) => Ok(Some(snapshot)),
        Err(InstallerError::Operation {
            operation: "task-read",
            ..
        }) => Ok(None),
        Err(error) => Err(error),
    }
}

/// Registers exact XML through `ITaskFolder::RegisterTask`.
pub fn register(xml: &str, enabled: bool) -> Result<(), InstallerError> {
    let _com = ComApartment::initialize()?;
    let service = service()?;
    let root = BSTR::from(r"\wifimic\");
    let root_folder = BSTR::from(r"\");
    // SAFETY: The folder path is a valid BSTR; the fallback creates the owned folder only.
    let folder = match unsafe { service.GetFolder(&root) } {
        Ok(folder) => folder,
        Err(_) => {
            let parent = unsafe { service.GetFolder(&root_folder) }.map_err(|error| {
                InstallerError::Operation {
                    operation: "task-root",
                    message: error.to_string(),
                }
            })?;
            let empty = VARIANT::default();
            unsafe { parent.CreateFolder(&BSTR::from("wifimic"), &empty) }.map_err(|error| {
                InstallerError::Operation {
                    operation: "task-folder",
                    message: error.to_string(),
                }
            })?
        }
    };
    let path = BSTR::from("wifimic-client");
    let xml = BSTR::from(xml);
    let empty = VARIANT::default();
    // SAFETY: All BSTR/VARIANT arguments remain alive for the COM call and the XML is caller-verified.
    let registered = unsafe {
        folder.RegisterTask(
            &path,
            &xml,
            TASK_CREATE_OR_UPDATE.0,
            &empty,
            &empty,
            TASK_LOGON_INTERACTIVE_TOKEN,
            &empty,
        )
    }
    .map_err(|error| InstallerError::Operation {
        operation: "task-register",
        message: error.to_string(),
    })?;
    // SAFETY: The returned task is live and owned by this scope.
    unsafe { registered.SetEnabled(enabled.into()) }.map_err(|error| InstallerError::Operation {
        operation: "task-enable",
        message: error.to_string(),
    })
}

/// Deletes the canonical task when it exists.
pub fn delete() -> Result<(), InstallerError> {
    let _com = ComApartment::initialize()?;
    let service = service()?;
    // SAFETY: The service is used only after successful COM initialization.
    let root = BSTR::from(r"\wifimic\");
    // SAFETY: The root BSTR is valid for the call.
    let folder =
        unsafe { service.GetFolder(&root) }.map_err(|error| InstallerError::Operation {
            operation: "task-root",
            message: error.to_string(),
        })?;
    let path = BSTR::from("wifimic-client");
    // SAFETY: Deleting the fixed owned task path cannot invoke a shell or external process.
    unsafe { folder.DeleteTask(&path, 0) }.map_err(|error| InstallerError::Operation {
        operation: "task-delete",
        message: error.to_string(),
    })
}

/// Disables and stops the canonical task, waiting for a non-running state.
pub fn disable_stop() -> Result<(), InstallerError> {
    with_task(|registered| {
        // SAFETY: The registered task is a live COM interface within the initialized apartment.
        unsafe { registered.SetEnabled(false.into()) }.map_err(|error| {
            InstallerError::Operation {
                operation: "task-disable",
                message: error.to_string(),
            }
        })?;
        // SAFETY: Stop(0) requests the current task instance to stop.
        unsafe { registered.Stop(0) }.map_err(|error| InstallerError::Operation {
            operation: "task-stop",
            message: error.to_string(),
        })
    })?;
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let state = snapshot()?.map_or(TASK_STATE_UNKNOWN, |value| {
            if value.running {
                TASK_STATE_RUNNING
            } else {
                TASK_STATE_READY
            }
        });
        if state != TASK_STATE_RUNNING {
            return Ok(());
        }
        if Instant::now() >= deadline {
            return Err(InstallerError::Operation {
                operation: "task-stop-wait",
                message: "task remained running".to_owned(),
            });
        }
        thread::sleep(Duration::from_millis(100));
    }
}

/// Enables and starts the canonical task.
pub fn enable_start() -> Result<(), InstallerError> {
    with_task(|registered| {
        // SAFETY: The task interface is live and owned by this initialized apartment.
        unsafe { registered.SetEnabled(true.into()) }.map_err(|error| {
            InstallerError::Operation {
                operation: "task-enable",
                message: error.to_string(),
            }
        })?;
        // SAFETY: An empty VARIANT requests the current task action without parameters.
        unsafe { registered.Run(&VARIANT::default()) }
            .map(|_| ())
            .map_err(|error| InstallerError::Operation {
                operation: "task-start",
                message: error.to_string(),
            })
    })
}

/// Returns whether the task is enabled and in Ready or Running state.
pub fn healthy() -> Result<bool, InstallerError> {
    let Some(value) = snapshot()? else {
        return Ok(false);
    };
    Ok(value.enabled && (value.running || task_state_ready()?))
}

fn task_state_ready() -> Result<bool, InstallerError> {
    with_task(|registered| {
        // SAFETY: The task interface is live for the property query.
        Ok(
            unsafe { registered.State() }.map_err(|error| InstallerError::Operation {
                operation: "task-state",
                message: error.to_string(),
            })? == TASK_STATE_READY,
        )
    })
}
