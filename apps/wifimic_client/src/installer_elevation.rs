//! Windows elevation and interactive-session guards for host mutation.

#![cfg(windows)]

use std::path::Path;
use windows::core::{BOOL, PCWSTR};
use windows::Win32::Foundation::{CloseHandle, HANDLE};
use windows::Win32::Security::{
    CheckTokenMembership, CreateWellKnownSid, WinBuiltinAdministratorsSid, PSID,
};
use windows::Win32::System::StationsAndDesktops::{
    GetProcessWindowStation, GetUserObjectInformationW, UOI_FLAGS,
};
use windows::Win32::System::Threading::{GetExitCodeProcess, WaitForSingleObject, INFINITE};
use windows::Win32::UI::Shell::{
    ShellExecuteExW, SEE_MASK_NOASYNC, SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW,
};
use windows::Win32::UI::WindowsAndMessaging::WSF_VISIBLE;

use crate::installer::InstallerError;

/// Checks the effective Administrators SID using `CheckTokenMembership`.
pub fn is_running_as_administrator() -> Result<bool, InstallerError> {
    let mut sid = [0_u8; 68];
    let mut length = sid.len() as u32;
    // SAFETY: `sid` is writable storage large enough for a well-known SID and length is its capacity.
    unsafe {
        CreateWellKnownSid(
            WinBuiltinAdministratorsSid,
            None,
            Some(PSID(sid.as_mut_ptr().cast())),
            &mut length,
        )
    }
    .map_err(|error| InstallerError::Preflight(error.to_string()))?;
    let mut member = BOOL::default();
    // SAFETY: The SID points to the initialized buffer returned by CreateWellKnownSid.
    unsafe { CheckTokenMembership(None, PSID(sid.as_mut_ptr().cast()), &mut member) }
        .map_err(|error| InstallerError::Preflight(error.to_string()))?;
    Ok(member.as_bool())
}

/// Checks that the process owns a visible interactive window station.
pub fn is_interactive_session() -> Result<bool, InstallerError> {
    // SAFETY: The current process window station handle is read-only and owned by the OS.
    let station = unsafe { GetProcessWindowStation() }
        .map_err(|error| InstallerError::Preflight(error.to_string()))?;
    if station.is_invalid() {
        return Err(InstallerError::Preflight(
            "GetProcessWindowStation failed".to_owned(),
        ));
    }
    let mut flags = 0_u32;
    let mut needed = 0_u32;
    // SAFETY: `flags` and `needed` are valid output buffers for UOI_FLAGS.
    unsafe {
        GetUserObjectInformationW(
            HANDLE(station.0),
            UOI_FLAGS,
            Some((&mut flags as *mut u32).cast()),
            std::mem::size_of::<u32>() as u32,
            Some(&mut needed),
        )
    }
    .map_err(|error| InstallerError::Preflight(error.to_string()))?;
    Ok(flags & WSF_VISIBLE as u32 != 0)
}

/// Launches an installer with UAC elevation and returns its stable process exit code.
pub fn relaunch_elevated(exe_path: &Path, args: &[String]) -> Result<u32, InstallerError> {
    let file = wide(exe_path.to_string_lossy().as_ref());
    let parameters = args
        .iter()
        .map(|arg| quote_windows_argument(arg))
        .collect::<Vec<_>>()
        .join(" ");
    let mut parameters = wide(&parameters);
    let verb = wide("runas");
    let mut info = SHELLEXECUTEINFOW {
        cbSize: std::mem::size_of::<SHELLEXECUTEINFOW>() as u32,
        fMask: SEE_MASK_NOCLOSEPROCESS | SEE_MASK_NOASYNC,
        lpVerb: PCWSTR(verb.as_ptr()),
        lpFile: PCWSTR(file.as_ptr()),
        lpParameters: PCWSTR(parameters.as_mut_ptr()),
        nShow: 0,
        ..Default::default()
    };
    // SAFETY: All strings are NUL-terminated and remain alive until ShellExecuteExW returns.
    unsafe { ShellExecuteExW(&mut info) }.map_err(|error| InstallerError::Operation {
        operation: "elevated-launch",
        message: error.to_string(),
    })?;
    let process = HANDLE(info.hProcess.0);
    // SAFETY: The returned process handle is valid while waiting and querying its exit code.
    unsafe { WaitForSingleObject(process, INFINITE) };
    let mut code = 1_u32;
    // SAFETY: The process has terminated and `code` is writable output storage.
    unsafe { GetExitCodeProcess(process, &mut code) }.map_err(|error| {
        InstallerError::Operation {
            operation: "elevated-exit-code",
            message: error.to_string(),
        }
    })?;
    // SAFETY: The process handle was returned by ShellExecuteExW and is closed exactly once.
    unsafe { CloseHandle(process) }.map_err(|error| InstallerError::Operation {
        operation: "elevated-close",
        message: error.to_string(),
    })?;
    Ok(code)
}

fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}
fn quote_windows_argument(value: &str) -> String {
    if value.is_empty() {
        return "\"\"".to_owned();
    }
    if !value
        .bytes()
        .any(|byte| byte == b' ' || byte == b'\t' || byte == b'\"')
    {
        return value.to_owned();
    }
    let mut output = String::from("\"");
    let mut slashes = 0;
    for character in value.chars() {
        if character == '\\' {
            slashes += 1;
            continue;
        }
        if character == '"' {
            output.push_str(&"\\".repeat(slashes * 2 + 1));
            output.push(character);
            slashes = 0;
            continue;
        }
        output.push_str(&"\\".repeat(slashes));
        slashes = 0;
        output.push(character);
    }
    output.push_str(&"\\".repeat(slashes * 2));
    output.push('"');
    output
}
