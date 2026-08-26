use std::fs::{self, File};
use std::io::{self, Cursor};
use std::path::{Component, Path, PathBuf};

use sha2::{Digest, Sha256};
use wifimic_update::download_release_asset;
use zip::ZipArchive;

use super::{UpdaterError, CLIENT_EXECUTABLE_NAME};

const ARCHIVE_NAME: &str = "wifimic-windows-x86_64.zip";
const CHECKSUM_NAME: &str = "wifimic-windows-x86_64.zip.sha256";

/// Downloads, verifies, and safely extracts the Windows client release.
pub fn download_and_verify_release(tag: &str) -> Result<PathBuf, UpdaterError> {
    let archive = download_release_asset(tag, ARCHIVE_NAME).map_err(|error| {
        UpdaterError::Download {
            message: error.to_string(),
        }
    })?;
    let manifest = download_release_asset(tag, CHECKSUM_NAME).map_err(|error| {
        UpdaterError::Download {
            message: error.to_string(),
        }
    })?;
    let expected = std::str::from_utf8(&manifest)
        .ok()
        .and_then(|value| value.split_whitespace().next())
        .filter(|value| value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit()))
        .map(str::to_ascii_lowercase)
        .ok_or(UpdaterError::InvalidChecksumManifest)?;
    let actual = format!("{:x}", Sha256::digest(&archive));
    if expected != actual {
        return Err(UpdaterError::ChecksumMismatch { expected, actual });
    }

    let staging = unique_staging_dir()?;
    validate_archive(&archive)?;
    extract_archive(&archive, &staging)?;
    let executable = staging.join(CLIENT_EXECUTABLE_NAME);
    let metadata = fs::metadata(executable).map_err(|_| UpdaterError::MissingExecutable)?;
    if !metadata.is_file() || metadata.len() == 0 {
        return Err(UpdaterError::MissingExecutable);
    }
    Ok(staging)
}

fn validate_archive(bytes: &[u8]) -> Result<(), UpdaterError> {
    let mut archive =
        ZipArchive::new(Cursor::new(bytes)).map_err(|error| UpdaterError::Archive {
            message: error.to_string(),
        })?;
    for index in 0..archive.len() {
        let entry = archive
            .by_index(index)
            .map_err(|error| UpdaterError::Archive {
                message: error.to_string(),
            })?;
        let path = Path::new(entry.name());
        if path.is_absolute()
            || path
                .components()
                .any(|component| matches!(component, Component::ParentDir | Component::Prefix(_)))
        {
            return Err(UpdaterError::Archive {
                message: format!("unsafe archive path {path:?}"),
            });
        }
    }
    Ok(())
}

fn extract_archive(bytes: &[u8], staging: &Path) -> Result<(), UpdaterError> {
    let mut archive =
        ZipArchive::new(Cursor::new(bytes)).map_err(|error| UpdaterError::Archive {
            message: error.to_string(),
        })?;
    for index in 0..archive.len() {
        let mut entry = archive
            .by_index(index)
            .map_err(|error| UpdaterError::Archive {
                message: error.to_string(),
            })?;
        let destination = staging.join(Path::new(entry.name()));
        if entry.is_dir() {
            fs::create_dir_all(&destination).map_err(|error| UpdaterError::Archive {
                message: error.to_string(),
            })?;
            continue;
        }
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent).map_err(|error| UpdaterError::Archive {
                message: error.to_string(),
            })?;
        }
        let mut output = File::create(destination).map_err(|error| UpdaterError::Archive {
            message: error.to_string(),
        })?;
        io::copy(&mut entry, &mut output).map_err(|error| UpdaterError::Archive {
            message: error.to_string(),
        })?;
    }
    Ok(())
}

fn unique_staging_dir() -> Result<PathBuf, UpdaterError> {
    let path = std::env::temp_dir().join(format!(
        "wifimic_client.stage.{}-{}",
        std::process::id(),
        super::timestamp()
    ));
    fs::create_dir(&path).map_err(|error| UpdaterError::Archive {
        message: error.to_string(),
    })?;
    Ok(path)
}
