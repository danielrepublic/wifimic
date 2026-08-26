//! Client-side release discovery, staging, and user-confirmed elevation.

#![cfg(windows)]

use std::fs::{self, File};
use std::path::Path;

use sha2::{Digest, Sha256};
use windows::core::PCWSTR;
use windows::Win32::UI::WindowsAndMessaging::{
    MessageBoxW, IDYES, MB_ICONINFORMATION, MB_ICONQUESTION, MB_OK, MB_YESNO, MESSAGEBOX_STYLE,
};
use zip::ZipArchive;

use wifimic_update::{
    compare_versions, discover_latest_tag, download_release_asset, is_release_tag,
    VersionComparison,
};

use wifimic_client::installer_elevation::relaunch_elevated;

const ARCHIVE_NAME: &str = "wifimic-windows-x86_64.zip";
const CHECKSUM_NAME: &str = "wifimic-windows-x86_64.zip.sha256";

/// Reports a read-only update check result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckOutcome {
    UpToDate { current: String, latest: String },
    UpdateAvailable { current: String, latest: String },
    CurrentNewer { current: String, latest: String },
}

/// Discovers and compares the current Windows client release.
pub fn check_update() -> Result<CheckOutcome, String> {
    let latest = discover_latest_tag().map_err(|error| error.to_string())?;
    match compare_versions(env!("WIFIMIC_CLIENT_VERSION"), &latest) {
        VersionComparison::UpToDate => Ok(CheckOutcome::UpToDate {
            current: env!("WIFIMIC_CLIENT_VERSION").to_owned(),
            latest,
        }),
        VersionComparison::UpdateAvailable => Ok(CheckOutcome::UpdateAvailable {
            current: env!("WIFIMIC_CLIENT_VERSION").to_owned(),
            latest,
        }),
        VersionComparison::CurrentNewer => Ok(CheckOutcome::CurrentNewer {
            current: env!("WIFIMIC_CLIENT_VERSION").to_owned(),
            latest,
        }),
        VersionComparison::Indeterminate => Err(format!(
            "cannot compare current version {:?}",
            env!("WIFIMIC_CLIENT_VERSION")
        )),
    }
}

/// Renders the Traditional Chinese one-line check result used by the server CLI.
pub fn render_check(outcome: &Result<CheckOutcome, String>) -> String {
    match outcome {
        Ok(CheckOutcome::UpToDate { current, .. }) => format!("目前版本 {current} 已是最新版本"),
        Ok(CheckOutcome::UpdateAvailable { current, latest }) => {
            format!("有新版本可用：{current} → {latest}，執行 `wifimic_client upgrade` 進行更新")
        }
        Ok(CheckOutcome::CurrentNewer { current, latest }) => {
            format!("目前版本 {current} 比最新版本 {latest} 更新")
        }
        Err(error) => format!("更新檢查失敗：{error}"),
    }
}

/// Downloads, verifies, extracts, and elevates the bundled installer.
pub fn upgrade(requested_tag: Option<&str>) -> Result<String, String> {
    let target = match requested_tag {
        Some(tag) if is_release_tag(tag) => tag.to_owned(),
        Some(tag) => return Err(format!("release tag {tag:?} is invalid")),
        None => discover_latest_tag().map_err(|error| error.to_string())?,
    };
    if requested_tag.is_none()
        && matches!(
            compare_versions(env!("WIFIMIC_CLIENT_VERSION"), &target),
            VersionComparison::UpToDate | VersionComparison::CurrentNewer
        )
    {
        return Ok("已是最新版本".to_owned());
    }
    let archive =
        download_release_asset(&target, ARCHIVE_NAME).map_err(|error| error.to_string())?;
    let manifest =
        download_release_asset(&target, CHECKSUM_NAME).map_err(|error| error.to_string())?;
    verify_checksum(&archive, &manifest)?;
    let stage = std::env::temp_dir().join(format!("wifimic-client-upgrade-{}", std::process::id()));
    fs::create_dir_all(&stage).map_err(|error| error.to_string())?;
    extract_asset(&archive, &stage, "wifimic_client.exe")?;
    extract_asset(&archive, &stage, "wifimic_client_installer.exe")?;
    let installer = stage.join("wifimic_client_installer.exe");
    let candidate = stage.join("wifimic_client.exe");
    let args = vec![
        "upgrade".to_owned(),
        "--client-executable".to_owned(),
        candidate.to_string_lossy().into_owned(),
        "--accept-host-mutation".to_owned(),
    ];
    let exit_code = relaunch_elevated(&installer, &args).map_err(|error| error.to_string())?;
    let _ = fs::remove_dir_all(&stage);
    match exit_code {
        0 => Ok(format!("已更新至 {target}")),
        10 => Err("安裝程式拒絕主機變更（需要系統管理員與互動式工作階段）".to_owned()),
        21 => Err("更新失敗，且回復也失敗".to_owned()),
        code => Err(format!("更新失敗，安裝程式結束碼 {code}")),
    }
}

/// Handles the tray action synchronously between message-loop ticks.
pub fn handle_tray_update() {
    // This is deliberately synchronous: it runs only after an explicit menu click and
    // returns to the bounded 50ms audio/message poll as soon as the dialog/operation ends.
    let outcome = check_update();
    if let Ok(CheckOutcome::UpdateAvailable { current, latest }) = &outcome {
        let message = wide(&format!(
            "有新版本可用：{current} → {latest}\n是否立即更新？"
        ));
        let title = wide("wifimic 更新");
        // SAFETY: MessageBoxW receives NUL-terminated strings alive for the call.
        let answer = unsafe {
            MessageBoxW(
                None,
                PCWSTR(message.as_ptr()),
                PCWSTR(title.as_ptr()),
                MB_YESNO | MB_ICONQUESTION,
            )
        };
        if answer == IDYES {
            let result = upgrade(Some(latest));
            show_result(&result.map(|_| "更新成功".to_owned()));
        }
    } else if let Ok(CheckOutcome::UpToDate { .. } | CheckOutcome::CurrentNewer { .. }) = outcome {
        show_message(
            "目前版本已是最新版本",
            "wifimic 更新",
            MB_OK | MB_ICONINFORMATION,
        );
    } else if let Err(error) = outcome {
        show_message(
            &format!("更新檢查失敗：{error}"),
            "wifimic 更新",
            MB_OK | MB_ICONINFORMATION,
        );
    }
}

fn verify_checksum(bytes: &[u8], manifest: &[u8]) -> Result<(), String> {
    let expected = std::str::from_utf8(manifest)
        .map_err(|error| error.to_string())?
        .split_whitespace()
        .next()
        .filter(|value| value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit()))
        .ok_or_else(|| "release checksum manifest is malformed".to_owned())?
        .to_ascii_lowercase();
    let actual = format!("{:x}", Sha256::digest(bytes));
    if expected == actual {
        Ok(())
    } else {
        Err(format!(
            "release checksum mismatch: expected {expected}, got {actual}"
        ))
    }
}
fn extract_asset(bytes: &[u8], stage: &Path, name: &str) -> Result<(), String> {
    let mut archive =
        ZipArchive::new(std::io::Cursor::new(bytes)).map_err(|error| error.to_string())?;
    let mut entry = archive.by_name(name).map_err(|error| error.to_string())?;
    if !entry.is_file() {
        return Err(format!("release archive entry {name:?} is not a file"));
    }
    let output = stage.join(name);
    let mut file = File::create(&output).map_err(|error| error.to_string())?;
    std::io::copy(&mut entry, &mut file).map_err(|error| error.to_string())?;
    Ok(())
}
fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}
fn show_message(message: &str, title: &str, flags: MESSAGEBOX_STYLE) {
    let message = wide(message);
    let title = wide(title); /* SAFETY: Both vectors stay alive for the call. */
    unsafe {
        MessageBoxW(
            None,
            PCWSTR(message.as_ptr()),
            PCWSTR(title.as_ptr()),
            flags,
        );
    }
}
fn show_result(result: &Result<String, String>) {
    match result {
        Ok(message) => show_message(message, "wifimic 更新", MB_OK | MB_ICONINFORMATION),
        Err(error) => show_message(error, "wifimic 更新", MB_OK | MB_ICONINFORMATION),
    }
}
