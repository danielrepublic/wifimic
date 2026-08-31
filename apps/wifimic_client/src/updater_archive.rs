use std::fs::{self, File};
use std::io::{self, Cursor};
use std::path::{Component, Path, PathBuf};

use wifimic_update::{download_release_asset, verify_release_fingerprint, TransactionError};
use zip::ZipArchive;

use crate::updater::CLIENT_EXECUTABLE_NAME;

const ARCHIVE_NAME: &str = "wifimic-windows-x86_64.zip";
const CHECKSUM_NAME: &str = "wifimic-windows-x86_64.zip.sha256";

/// Downloads, verifies, and safely extracts the Windows client release.
///
/// Any failure after creating the staging directory removes that directory so
/// the transaction engine never needs to clean an artifact it did not receive.
pub fn download_and_verify_release(tag: &str) -> Result<PathBuf, TransactionError> {
    let archive = download_release_asset(tag, ARCHIVE_NAME).map_err(stage_error)?;
    let manifest = download_release_asset(tag, CHECKSUM_NAME).map_err(stage_error)?;
    verify_release_fingerprint(&archive, &manifest).map_err(stage_error)?;

    let staging = unique_staging_dir()?;
    let result = (|| {
        validate_archive(&archive)?;
        extract_archive(&archive, &staging)?;
        let executable = staging.join(CLIENT_EXECUTABLE_NAME);
        let metadata = fs::metadata(executable).map_err(|_| missing_executable())?;
        if metadata.is_file() && metadata.len() > 0 {
            Ok(())
        } else {
            Err(missing_executable())
        }
    })();
    match result {
        Ok(()) => Ok(staging),
        Err(error) => {
            let _ = fs::remove_dir_all(&staging);
            Err(error)
        }
    }
}

fn validate_archive(bytes: &[u8]) -> Result<(), TransactionError> {
    let mut archive = ZipArchive::new(Cursor::new(bytes)).map_err(stage_error)?;
    for index in 0..archive.len() {
        let entry = archive.by_index(index).map_err(stage_error)?;
        let path = Path::new(entry.name());
        if path.is_absolute()
            || path
                .components()
                .any(|component| matches!(component, Component::ParentDir | Component::Prefix(_)))
        {
            return Err(TransactionError::Stage {
                message: format!("unsafe archive path {path:?}"),
            });
        }
    }
    Ok(())
}

fn extract_archive(bytes: &[u8], staging: &Path) -> Result<(), TransactionError> {
    let mut archive = ZipArchive::new(Cursor::new(bytes)).map_err(stage_error)?;
    for index in 0..archive.len() {
        let mut entry = archive.by_index(index).map_err(stage_error)?;
        let destination = staging.join(Path::new(entry.name()));
        if entry.is_dir() {
            fs::create_dir_all(&destination).map_err(stage_error)?;
            continue;
        }
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent).map_err(stage_error)?;
        }
        let mut output = File::create(destination).map_err(stage_error)?;
        io::copy(&mut entry, &mut output).map_err(stage_error)?;
    }
    Ok(())
}

fn unique_staging_dir() -> Result<PathBuf, TransactionError> {
    let path = std::env::temp_dir().join(format!(
        "wifimic_client.stage.{}-{}",
        std::process::id(),
        timestamp()
    ));
    fs::create_dir(&path).map_err(stage_error)?;
    Ok(path)
}

fn missing_executable() -> TransactionError {
    TransactionError::Stage {
        message: "release archive does not contain a non-empty wifimic_client.exe".to_owned(),
    }
}

fn stage_error(error: impl std::fmt::Display) -> TransactionError {
    TransactionError::Stage {
        message: error.to_string(),
    }
}

fn timestamp() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |duration| duration.as_nanos())
}
