use std::fs;
use std::io::Cursor;
use std::path::{Component, Path, PathBuf};
use std::process::Command;
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use flate2::read::GzDecoder;
use sha2::{Digest, Sha256};
use tar::Archive;
use wifimic_update::download_release_asset;

use crate::upgrade::UpgradeError;

const ARCHIVE_NAME: &str = "wifimic-linux-x86_64.tar.gz";
const CHECKSUM_NAME: &str = "wifimic-linux-x86_64.tar.gz.sha256";

pub(crate) fn download_and_verify(tag: &str) -> Result<PathBuf, UpgradeError> {
    let archive =
        download_release_asset(tag, ARCHIVE_NAME).map_err(|error| UpgradeError::Download {
            message: error.to_string(),
        })?;
    let manifest =
        download_release_asset(tag, CHECKSUM_NAME).map_err(|error| UpgradeError::Download {
            message: error.to_string(),
        })?;
    let expected = std::str::from_utf8(&manifest)
        .ok()
        .and_then(|value| value.split_whitespace().next())
        .filter(|value| value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit()))
        .map(str::to_ascii_lowercase)
        .ok_or(UpgradeError::InvalidChecksumManifest)?;
    let actual = format!("{:x}", Sha256::digest(&archive));
    if expected != actual {
        return Err(UpgradeError::ChecksumMismatch { expected, actual });
    }
    let staging = unique_staging_dir()?;
    validate_archive(&archive)?;
    Archive::new(GzDecoder::new(Cursor::new(&archive)))
        .unpack(&staging)
        .map_err(|error| UpgradeError::Archive {
            message: error.to_string(),
        })?;
    let binary = staging.join("wifimic_server");
    let metadata = fs::metadata(binary).map_err(|_| UpgradeError::MissingBinary)?;
    if !metadata.is_file() || metadata.len() == 0 {
        return Err(UpgradeError::MissingBinary);
    }
    Ok(staging)
}

pub(crate) fn install_path() -> Result<PathBuf, UpgradeError> {
    std::env::current_exe()
        .map_err(|error| UpgradeError::InstallPath {
            message: error.to_string(),
        })
        .and_then(|path| {
            fs::canonicalize(path).map_err(|error| UpgradeError::InstallPath {
                message: error.to_string(),
            })
        })
}

pub(crate) fn backup_current_binary(backup_path: &Path) -> Result<(), UpgradeError> {
    let current = install_path()?;
    fs::copy(current, backup_path)
        .map(|_| ())
        .map_err(|error| UpgradeError::Backup {
            message: error.to_string(),
        })
}

pub(crate) fn stop_service() -> Result<(), UpgradeError> {
    run_systemctl("stop").map_err(|message| UpgradeError::Stop { message })
}

pub(crate) fn restart_service() -> Result<(), UpgradeError> {
    run_systemctl("restart").map_err(|message| UpgradeError::Restart { message })
}

pub(crate) fn atomic_swap(staged_binary: &Path, install_path: &Path) -> Result<(), UpgradeError> {
    let temporary = install_path.with_file_name(format!(
        ".wifimic_server.upgrade-{}-{}",
        std::process::id(),
        timestamp()
    ));
    fs::copy(staged_binary, &temporary).map_err(|error| UpgradeError::Swap {
        message: error.to_string(),
    })?;
    fs::rename(&temporary, install_path).map_err(|error| UpgradeError::Swap {
        message: error.to_string(),
    })
}

pub(crate) fn wait_for_active(timeout: Duration) -> Result<bool, UpgradeError> {
    let deadline = Instant::now() + timeout;
    loop {
        let output = Command::new("systemctl")
            .args(["--user", "is-active", "wifimic-server"])
            .output()
            .map_err(|error| UpgradeError::HealthQuery {
                message: error.to_string(),
            })?;
        if output.status.success() && String::from_utf8_lossy(&output.stdout).trim() == "active" {
            return Ok(true);
        }
        if Instant::now() >= deadline {
            return Ok(false);
        }
        thread::sleep(Duration::from_millis(250));
    }
}

pub(crate) fn restore_backup(backup_path: &Path, install_path: &Path) -> Result<(), UpgradeError> {
    let temporary = install_path.with_file_name(format!(
        ".wifimic_server.rollback-{}-{}",
        std::process::id(),
        timestamp()
    ));
    fs::copy(backup_path, &temporary).map_err(|error| UpgradeError::Backup {
        message: error.to_string(),
    })?;
    fs::rename(&temporary, install_path).map_err(|error| UpgradeError::Backup {
        message: error.to_string(),
    })
}

fn run_systemctl(action: &str) -> Result<(), String> {
    let output = Command::new("systemctl")
        .args(["--user", action, "wifimic-server"])
        .output()
        .map_err(|error| error.to_string())?;
    if output.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).trim().to_owned())
    }
}

fn validate_archive(bytes: &[u8]) -> Result<(), UpgradeError> {
    let mut archive = Archive::new(GzDecoder::new(Cursor::new(bytes)));
    let entries = archive.entries().map_err(|error| UpgradeError::Archive {
        message: error.to_string(),
    })?;
    for entry in entries {
        let entry = entry.map_err(|error| UpgradeError::Archive {
            message: error.to_string(),
        })?;
        let path = entry.path().map_err(|error| UpgradeError::Archive {
            message: error.to_string(),
        })?;
        if path.is_absolute()
            || path
                .components()
                .any(|component| matches!(component, Component::ParentDir | Component::Prefix(_)))
        {
            return Err(UpgradeError::Archive {
                message: format!("unsafe archive path {path:?}"),
            });
        }
    }
    Ok(())
}

fn unique_staging_dir() -> Result<PathBuf, UpgradeError> {
    let path = std::env::temp_dir().join(format!(
        "wifimic_server.stage.{}-{}",
        std::process::id(),
        timestamp()
    ));
    fs::create_dir(&path).map_err(|error| UpgradeError::Archive {
        message: error.to_string(),
    })?;
    Ok(path)
}

fn timestamp() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_nanos())
}
