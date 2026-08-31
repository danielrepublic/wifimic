use std::fs;
use std::io::Cursor;
use std::path::{Component, Path, PathBuf};
use std::process::Command;
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use flate2::read::GzDecoder;
use tar::Archive;
use wifimic_update::{download_release_asset, verify_release_fingerprint, TransactionError};

const ARCHIVE_NAME: &str = "wifimic-linux-x86_64.tar.gz";
const CHECKSUM_NAME: &str = "wifimic-linux-x86_64.tar.gz.sha256";

pub(crate) fn download_and_verify(tag: &str) -> Result<PathBuf, TransactionError> {
    let archive = download_release_asset(tag, ARCHIVE_NAME).map_err(stage_error)?;
    let manifest = download_release_asset(tag, CHECKSUM_NAME).map_err(stage_error)?;
    verify_release_fingerprint(&archive, &manifest).map_err(stage_error)?;
    let staging = unique_staging_dir().map_err(stage_error)?;
    let result = extract_archive(&archive, &staging);
    if result.is_err() {
        let _ = fs::remove_dir_all(&staging);
    }
    result.map_err(stage_error)?;
    Ok(staging)
}

pub(crate) fn install_path() -> Result<PathBuf, String> {
    std::env::current_exe()
        .map_err(|error| error.to_string())
        .and_then(|path| fs::canonicalize(path).map_err(|error| error.to_string()))
}

pub(crate) fn backup_current_binary(install_path: &Path, backup_path: &Path) -> Result<(), String> {
    let result = fs::copy(install_path, backup_path)
        .map(|_| ())
        .map_err(|error| error.to_string());
    if result.is_err() {
        let _ = fs::remove_file(backup_path);
    }
    result
}

pub(crate) fn stop_service() -> Result<(), String> {
    run_systemctl("stop")
}

pub(crate) fn restart_service() -> Result<(), String> {
    run_systemctl("restart")
}

pub(crate) fn atomic_swap(staged_binary: &Path, install_path: &Path) -> Result<(), String> {
    let temporary = install_path.with_file_name(format!(
        ".wifimic_server.upgrade-{}-{}",
        std::process::id(),
        timestamp()
    ));
    fs::copy(staged_binary, &temporary).map_err(|error| error.to_string())?;
    fs::rename(&temporary, install_path).map_err(|error| error.to_string())
}

pub(crate) fn wait_for_active(timeout: Duration) -> Result<bool, String> {
    let deadline = Instant::now() + timeout;
    loop {
        let output = Command::new("systemctl")
            .args(["--user", "is-active", "wifimic-server"])
            .output()
            .map_err(|error| error.to_string())?;
        if output.status.success() && String::from_utf8_lossy(&output.stdout).trim() == "active" {
            return Ok(true);
        }
        if Instant::now() >= deadline {
            return Ok(false);
        }
        thread::sleep(Duration::from_millis(250));
    }
}

pub(crate) fn restore_backup(backup_path: &Path, install_path: &Path) -> Result<(), String> {
    let temporary = install_path.with_file_name(format!(
        ".wifimic_server.rollback-{}-{}",
        std::process::id(),
        timestamp()
    ));
    fs::copy(backup_path, &temporary).map_err(|error| error.to_string())?;
    fs::rename(&temporary, install_path).map_err(|error| error.to_string())
}

fn stage_error(error: impl std::fmt::Display) -> TransactionError {
    TransactionError::Stage {
        message: error.to_string(),
    }
}

fn extract_archive(archive_bytes: &[u8], staging: &Path) -> Result<(), String> {
    validate_archive(archive_bytes)?;
    Archive::new(GzDecoder::new(Cursor::new(archive_bytes)))
        .unpack(staging)
        .map_err(|error| error.to_string())?;
    let metadata =
        fs::metadata(staging.join("wifimic_server")).map_err(|error| error.to_string())?;
    if !metadata.is_file() || metadata.len() == 0 {
        return Err(
            "release archive does not contain a non-empty wifimic_server binary".to_owned(),
        );
    }
    Ok(())
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

fn validate_archive(bytes: &[u8]) -> Result<(), String> {
    let mut archive = Archive::new(GzDecoder::new(Cursor::new(bytes)));
    let entries = archive.entries().map_err(|error| error.to_string())?;
    for entry in entries {
        let entry = entry.map_err(|error| error.to_string())?;
        let path = entry.path().map_err(|error| error.to_string())?;
        if path.is_absolute()
            || path
                .components()
                .any(|component| matches!(component, Component::ParentDir | Component::Prefix(_)))
        {
            return Err(format!("unsafe archive path {path:?}"));
        }
    }
    Ok(())
}

fn unique_staging_dir() -> Result<PathBuf, String> {
    let path = std::env::temp_dir().join(format!(
        "wifimic_server.stage.{}-{}",
        std::process::id(),
        timestamp()
    ));
    fs::create_dir(&path).map_err(|error| error.to_string())?;
    Ok(path)
}

fn timestamp() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_nanos())
}
