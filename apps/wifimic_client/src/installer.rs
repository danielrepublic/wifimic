//! Transactional Windows installation and upgrade orchestration.

#![cfg(windows)]

use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use sha2::{Digest, Sha256};

/// The canonical Windows client installation directory.
pub const INSTALL_ROOT: &str = r"C:\Program Files\wifimic-client";
/// The canonical client executable name.
pub const EXECUTABLE_NAME: &str = "wifimic_client.exe";
/// The canonical Scheduled Task path.
pub const TASK_PATH: &str = r"\wifimic\wifimic-client";
/// The stable firewall rule name.
pub const FIREWALL_NAME: &str = "wifimic-client";
/// The peer address permitted by the firewall rule.
pub const PEER_ADDRESS: &str = "192.168.0.210/32";
/// The UDP port used by the client.
pub const UDP_PORT: &str = "6902";
/// The exact VB-CABLE endpoint required by the client.
pub const RENDER_ENDPOINT: &str = "CABLE Input (VB-Audio Virtual Cable)";
/// The maximum health-check duration.
pub const HEALTH_TIMEOUT: Duration = Duration::from_secs(45);

/// A semantic snapshot of the owned Scheduled Task plus its exact XML backup.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskSnapshot {
    /// The raw XML returned by Task Scheduler.
    pub xml: String,
    /// Whether the task was enabled when captured.
    pub enabled: bool,
    /// Whether the task was running when captured.
    pub running: bool,
}

/// A semantic snapshot of the owned firewall rule.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FirewallSnapshot {
    /// The stable rule name.
    pub name: String,
    /// The display label.
    pub display_name: String,
    /// The normalized remote IPv4 address.
    pub remote_address: String,
    /// Whether the rule is enabled.
    pub enabled: bool,
}

/// The captured executable bytes and their digest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutableSnapshot {
    /// The exact file bytes captured before mutation.
    pub bytes: Vec<u8>,
    /// The SHA-256 digest of [`Self::bytes`].
    pub sha256: String,
}

/// The installer configuration supplied by the thin installer binary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallerConfig {
    /// The source executable to install or stage.
    pub client_executable: PathBuf,
    /// The endpoint name checked during a fresh install.
    pub render_endpoint: String,
}

/// Reports installation failures and preserves the original/rollback boundary.
#[derive(Debug, thiserror::Error)]
pub enum InstallerError {
    /// Host mutation was refused before any side effect.
    #[error("host mutation preflight failed: {0}")]
    Preflight(String),
    /// A native or fake operation failed at a named seam.
    #[error("installer operation {operation} failed: {message}")]
    Operation {
        operation: &'static str,
        message: String,
    },
    /// A downloaded asset did not match its manifest.
    #[error("release checksum mismatch: expected {expected}, got {actual}")]
    ChecksumMismatch { expected: String, actual: String },
    /// The release checksum manifest was malformed.
    #[error("release checksum manifest is malformed")]
    InvalidChecksumManifest,
    /// The original mutation failure and rollback failure are both reported.
    #[error("installer failed: {operation}; rollback failed: {rollback}")]
    Rollback {
        operation: Box<Self>,
        rollback: Box<Self>,
    },
    /// The candidate executable was not a usable file.
    #[error("candidate executable is missing or empty: {0}")]
    InvalidCandidate(PathBuf),
}

/// Provides every host side effect needed by install and upgrade.
pub trait InstallerOperations {
    /// Performs administrator and interactive-session checks.
    fn check_preflight(&mut self) -> Result<(), InstallerError>;
    /// Reads the current owned task, if present.
    fn get_current_task(&mut self) -> Result<Option<TaskSnapshot>, InstallerError>;
    /// Reads the current owned firewall rule, if present.
    fn get_current_firewall(&mut self) -> Result<Option<FirewallSnapshot>, InstallerError>;
    /// Captures the current executable, if present.
    fn get_current_executable_hash(&mut self)
        -> Result<Option<ExecutableSnapshot>, InstallerError>;
    /// Stages and verifies a candidate download.
    fn stage_and_verify_download(&mut self, source: &Path) -> Result<PathBuf, InstallerError>;
    /// Registers the canonical task from XML and verifies its semantic contract.
    fn register_task(&mut self, xml: &str, enabled: bool) -> Result<(), InstallerError>;
    /// Creates or replaces the canonical firewall rule.
    fn set_firewall(&mut self) -> Result<(), InstallerError>;
    /// Disables and stops the current task.
    fn disable_stop_task(&mut self) -> Result<(), InstallerError>;
    /// Atomically swaps a same-directory staged executable into place.
    fn atomic_swap_executable(&mut self, staged: &Path) -> Result<(), InstallerError>;
    /// Enables and starts the task.
    fn enable_start_task(&mut self) -> Result<(), InstallerError>;
    /// Waits until the task and endpoint are healthy.
    fn wait_for_healthy(&mut self, timeout: Duration) -> Result<(), InstallerError>;
    /// Restores the captured task, or removes a newly-created task.
    fn restore_task(&mut self, snapshot: Option<&TaskSnapshot>) -> Result<(), InstallerError>;
    /// Restores the captured firewall, or removes a newly-created rule.
    fn restore_firewall(
        &mut self,
        snapshot: Option<&FirewallSnapshot>,
    ) -> Result<(), InstallerError>;
    /// Restores the captured executable, or removes a newly-created file.
    fn restore_executable(
        &mut self,
        snapshot: Option<&ExecutableSnapshot>,
    ) -> Result<(), InstallerError>;
}

/// Runs a fresh install with rollback after every post-mutation failure.
pub fn run_install<O: InstallerOperations>(
    operations: &mut O,
    config: &InstallerConfig,
) -> Result<(), InstallerError> {
    operations.check_preflight()?;
    let prior_task = operations.get_current_task()?;
    let prior_firewall = operations.get_current_firewall()?;
    let prior_executable = operations.get_current_executable_hash()?;
    let staged = operations.stage_and_verify_download(&config.client_executable)?;
    let result = (|| {
        operations.atomic_swap_executable(&staged)?;
        operations.register_task("canonical", true)?;
        operations.set_firewall()?;
        operations.enable_start_task()?;
        operations.wait_for_healthy(HEALTH_TIMEOUT)
    })();
    match result {
        Ok(()) => Ok(()),
        Err(operation) => Err(with_rollback(
            operations,
            operation,
            prior_task.as_ref(),
            prior_firewall.as_ref(),
            prior_executable.as_ref(),
        )),
    }
}

/// Runs an upgrade while preserving and restoring exact prior state on failure.
pub fn run_upgrade<O: InstallerOperations>(
    operations: &mut O,
    config: &InstallerConfig,
) -> Result<(), InstallerError> {
    operations.check_preflight()?;
    let prior_task = operations
        .get_current_task()?
        .ok_or_else(|| InstallerError::Preflight("canonical task is missing".to_owned()))?;
    let prior_firewall = operations.get_current_firewall()?;
    let prior_executable = operations
        .get_current_executable_hash()?
        .ok_or_else(|| InstallerError::Preflight("installed executable is missing".to_owned()))?;
    let staged = operations.stage_and_verify_download(&config.client_executable)?;
    let result = (|| {
        operations.disable_stop_task()?;
        operations.atomic_swap_executable(&staged)?;
        operations.register_task(&prior_task.xml, true)?;
        operations.enable_start_task()?;
        operations.wait_for_healthy(HEALTH_TIMEOUT)
    })();
    match result {
        Ok(()) => Ok(()),
        Err(operation) => Err(with_rollback(
            operations,
            operation,
            Some(&prior_task),
            prior_firewall.as_ref(),
            Some(&prior_executable),
        )),
    }
}

fn with_rollback<O: InstallerOperations>(
    operations: &mut O,
    operation: InstallerError,
    task: Option<&TaskSnapshot>,
    firewall: Option<&FirewallSnapshot>,
    executable: Option<&ExecutableSnapshot>,
) -> InstallerError {
    let task_result = operations.restore_task(task);
    let firewall_result = operations.restore_firewall(firewall);
    let executable_result = operations.restore_executable(executable);
    let rollback = [task_result, firewall_result, executable_result]
        .into_iter()
        .find_map(Result::err);
    match rollback {
        Some(rollback) => InstallerError::Rollback {
            operation: Box::new(operation),
            rollback: Box::new(rollback),
        },
        None => operation,
    }
}

/// Verifies a downloaded archive against the release's whitespace-delimited SHA-256 manifest.
pub fn verify_sha256(bytes: &[u8], manifest: &[u8]) -> Result<(), InstallerError> {
    let expected = std::str::from_utf8(manifest)
        .ok()
        .and_then(|text| text.split_whitespace().next())
        .filter(|digest| digest.len() == 64 && digest.bytes().all(|byte| byte.is_ascii_hexdigit()))
        .map(str::to_ascii_lowercase)
        .ok_or(InstallerError::InvalidChecksumManifest)?;
    let actual = format!("{:x}", Sha256::digest(bytes));
    if expected != actual {
        return Err(InstallerError::ChecksumMismatch { expected, actual });
    }
    Ok(())
}

/// Copies a candidate into a private staging path and verifies it is non-empty.
pub fn stage_candidate(source: &Path) -> Result<PathBuf, InstallerError> {
    let metadata =
        fs::metadata(source).map_err(|_| InstallerError::InvalidCandidate(source.to_owned()))?;
    if !metadata.is_file() || metadata.len() == 0 {
        return Err(InstallerError::InvalidCandidate(source.to_owned()));
    }
    let staging =
        std::env::temp_dir().join(format!("wifimic-client-installer-{}", std::process::id()));
    fs::create_dir_all(&staging).map_err(|error| InstallerError::Operation {
        operation: "stage",
        message: error.to_string(),
    })?;
    let target = staging.join(EXECUTABLE_NAME);
    fs::copy(source, &target).map_err(|error| InstallerError::Operation {
        operation: "stage",
        message: error.to_string(),
    })?;
    Ok(target)
}

/// Uses the real Windows adapters for installer host operations.
#[derive(Debug, Default)]
pub struct NativeInstallerOperations;

impl InstallerOperations for NativeInstallerOperations {
    fn check_preflight(&mut self) -> Result<(), InstallerError> {
        if !crate::installer_elevation::is_running_as_administrator()? {
            return Err(InstallerError::Preflight(
                "Administrator rights are required".to_owned(),
            ));
        }
        if !crate::installer_elevation::is_interactive_session()? {
            return Err(InstallerError::Preflight(
                "an interactive session is required".to_owned(),
            ));
        }
        Ok(())
    }

    fn get_current_task(&mut self) -> Result<Option<TaskSnapshot>, InstallerError> {
        crate::installer_task::snapshot()
    }
    fn get_current_firewall(&mut self) -> Result<Option<FirewallSnapshot>, InstallerError> {
        crate::installer_firewall::snapshot()
    }
    fn get_current_executable_hash(
        &mut self,
    ) -> Result<Option<ExecutableSnapshot>, InstallerError> {
        let path = Path::new(INSTALL_ROOT).join(EXECUTABLE_NAME);
        match fs::read(&path) {
            Ok(bytes) => Ok(Some(ExecutableSnapshot {
                sha256: format!("{:x}", Sha256::digest(&bytes)),
                bytes,
            })),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(InstallerError::Operation {
                operation: "read-executable",
                message: error.to_string(),
            }),
        }
    }

    fn stage_and_verify_download(&mut self, source: &Path) -> Result<PathBuf, InstallerError> {
        stage_candidate(source)
    }

    fn register_task(&mut self, xml: &str, enabled: bool) -> Result<(), InstallerError> {
        let canonical = canonical_task_xml();
        crate::installer_task::register(if xml == "canonical" { &canonical } else { xml }, enabled)
    }

    fn set_firewall(&mut self) -> Result<(), InstallerError> {
        crate::installer_firewall::set()
    }
    fn disable_stop_task(&mut self) -> Result<(), InstallerError> {
        crate::installer_task::disable_stop()
    }

    fn atomic_swap_executable(&mut self, staged: &Path) -> Result<(), InstallerError> {
        let target = Path::new(INSTALL_ROOT).join(EXECUTABLE_NAME);
        fs::create_dir_all(INSTALL_ROOT).map_err(|error| InstallerError::Operation {
            operation: "create-install-root",
            message: error.to_string(),
        })?;
        let temporary =
            target.with_file_name(format!(".{EXECUTABLE_NAME}.stage-{}", std::process::id()));
        fs::copy(staged, &temporary).map_err(|error| InstallerError::Operation {
            operation: "copy-staged-executable",
            message: error.to_string(),
        })?;
        if target.exists() {
            let backup =
                target.with_file_name(format!(".{EXECUTABLE_NAME}.backup-{}", std::process::id()));
            let target_wide = wide_path(&target);
            let temporary_wide = wide_path(&temporary);
            let backup_wide = wide_path(&backup);
            // SAFETY: All paths are absolute same-directory UTF-16 strings and the backup is real.
            let result = unsafe {
                windows::Win32::Storage::FileSystem::ReplaceFileW(
                    windows::core::PCWSTR(target_wide.as_ptr()),
                    windows::core::PCWSTR(temporary_wide.as_ptr()),
                    windows::core::PCWSTR(backup_wide.as_ptr()),
                    windows::Win32::Storage::FileSystem::REPLACE_FILE_FLAGS(0),
                    None,
                    None,
                )
            };
            let _ = fs::remove_file(&temporary);
            let _ = fs::remove_file(&backup);
            result.map_err(|error| InstallerError::Operation {
                operation: "replace-executable",
                message: error.to_string(),
            })
        } else {
            fs::rename(&temporary, &target).map_err(|error| InstallerError::Operation {
                operation: "install-executable",
                message: error.to_string(),
            })
        }
    }

    fn enable_start_task(&mut self) -> Result<(), InstallerError> {
        crate::installer_task::enable_start()
    }

    fn wait_for_healthy(&mut self, timeout: Duration) -> Result<(), InstallerError> {
        let deadline = std::time::Instant::now() + timeout;
        loop {
            let task_healthy = crate::installer_task::healthy()?;
            let endpoint_healthy = crate::render::enumerate_render_endpoints()
                .map(|names| names.iter().any(|name| name == RENDER_ENDPOINT))
                .map_err(|error| InstallerError::Operation {
                    operation: "render-endpoint",
                    message: error.to_string(),
                })?;
            if task_healthy && endpoint_healthy {
                return Ok(());
            }
            if std::time::Instant::now() >= deadline {
                return Err(InstallerError::Operation {
                    operation: "health",
                    message: "task or VB-CABLE endpoint did not become healthy".to_owned(),
                });
            }
            std::thread::sleep(Duration::from_millis(250));
        }
    }

    fn restore_task(&mut self, snapshot: Option<&TaskSnapshot>) -> Result<(), InstallerError> {
        match snapshot {
            Some(value) => crate::installer_task::register(&value.xml, value.enabled),
            None => crate::installer_task::delete(),
        }
    }
    fn restore_firewall(
        &mut self,
        snapshot: Option<&FirewallSnapshot>,
    ) -> Result<(), InstallerError> {
        match snapshot {
            Some(_) => crate::installer_firewall::set(),
            None => crate::installer_firewall::remove(),
        }
    }
    fn restore_executable(
        &mut self,
        snapshot: Option<&ExecutableSnapshot>,
    ) -> Result<(), InstallerError> {
        let target = Path::new(INSTALL_ROOT).join(EXECUTABLE_NAME);
        match snapshot {
            Some(value) => {
                let temporary = target.with_file_name(format!(
                    ".{EXECUTABLE_NAME}.rollback-{}",
                    std::process::id()
                ));
                fs::write(&temporary, &value.bytes).map_err(|error| InstallerError::Operation {
                    operation: "restore-executable",
                    message: error.to_string(),
                })?;
                if target.exists() {
                    let backup = target.with_file_name(format!(
                        ".{EXECUTABLE_NAME}.rollback-backup-{}",
                        std::process::id()
                    ));
                    let target_wide = wide_path(&target);
                    let temporary_wide = wide_path(&temporary);
                    let backup_wide = wide_path(&backup);
                    // SAFETY: The paths are absolute same-directory UTF-16 strings and backup is non-null.
                    unsafe {
                        windows::Win32::Storage::FileSystem::ReplaceFileW(
                            windows::core::PCWSTR(target_wide.as_ptr()),
                            windows::core::PCWSTR(temporary_wide.as_ptr()),
                            windows::core::PCWSTR(backup_wide.as_ptr()),
                            windows::Win32::Storage::FileSystem::REPLACE_FILE_FLAGS(0),
                            None,
                            None,
                        )
                    }
                    .map_err(|error| InstallerError::Operation {
                        operation: "restore-executable",
                        message: error.to_string(),
                    })?;
                    let _ = fs::remove_file(backup);
                } else {
                    fs::rename(&temporary, &target).map_err(|error| InstallerError::Operation {
                        operation: "restore-executable",
                        message: error.to_string(),
                    })?;
                }
                let restored = fs::read(&target).map_err(|error| InstallerError::Operation {
                    operation: "verify-executable",
                    message: error.to_string(),
                })?;
                if restored != value.bytes {
                    return Err(InstallerError::Operation {
                        operation: "verify-executable",
                        message: "restored executable hash differs".to_owned(),
                    });
                }
                Ok(())
            }
            None => fs::remove_file(target)
                .or_else(|error| {
                    if error.kind() == std::io::ErrorKind::NotFound {
                        Ok(())
                    } else {
                        Err(error)
                    }
                })
                .map_err(|error| InstallerError::Operation {
                    operation: "remove-executable",
                    message: error.to_string(),
                }),
        }
    }
}

fn wide_path(path: &Path) -> Vec<u16> {
    path.as_os_str()
        .to_string_lossy()
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect()
}

fn canonical_task_xml() -> String {
    let path = Path::new(INSTALL_ROOT)
        .join(EXECUTABLE_NAME)
        .to_string_lossy()
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;");
    format!(
        r#"<?xml version="1.0" encoding="UTF-16"?><Task xmlns="http://schemas.microsoft.com/windows/2004/02/mit/task" version="1.4"><RegistrationInfo><URI>{TASK_PATH}</URI></RegistrationInfo><Triggers><LogonTrigger><Enabled>true</Enabled></LogonTrigger></Triggers><Principals><Principal id="Author"><LogonType>InteractiveToken</LogonType><RunLevel>LeastPrivilege</RunLevel></Principal></Principals><Actions Context="Author"><Exec><Command>{path}</Command><WorkingDirectory>{INSTALL_ROOT}</WorkingDirectory></Exec></Actions></Task>"#
    )
}

#[cfg(test)]
#[path = "installer_test_support.rs"]
mod test_support;

#[cfg(test)]
#[path = "installer_tests.rs"]
mod tests;
