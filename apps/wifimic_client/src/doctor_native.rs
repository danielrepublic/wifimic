use std::path::Path;
use std::process::{Command, Output};

use super::{
    DoctorQueryError, FirewallQuery, FirewallSignature, InstallQuery, RenderEndpointQuery,
    CLIENT_INSTALL_PATH, EXPECTED_FIREWALL_DISPLAY_NAME,
};

/// Checks the installed executable at the fixed canonical install path.
#[derive(Debug, Default)]
pub(crate) struct NativeInstallQuery;

impl InstallQuery for NativeInstallQuery {
    fn executable_exists(&self) -> bool {
        Path::new(CLIENT_INSTALL_PATH).is_file()
    }
}

/// Enumerates render endpoints using the real WASAPI backend.
#[derive(Debug, Default)]
pub(crate) struct NativeRenderEndpointQuery;

impl RenderEndpointQuery for NativeRenderEndpointQuery {
    fn enumerate(&self) -> Result<Vec<String>, DoctorQueryError> {
        wifimic_client::render::enumerate_render_endpoints().map_err(|error| {
            DoctorQueryError::Invoke {
                operation: "enumerate_render_endpoints",
                message: error.to_string(),
            }
        })
    }
}

/// Queries the canonical firewall rule via `Get-NetFirewallRule`.
#[derive(Debug, Default)]
pub(crate) struct NativeFirewallQuery;

impl FirewallQuery for NativeFirewallQuery {
    fn signature(&self) -> Result<FirewallSignature, DoctorQueryError> {
        let output = Command::new("powershell.exe")
            .args([
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                &format!(
                    r#"$rule = Get-NetFirewallRule -DisplayName '{EXPECTED_FIREWALL_DISPLAY_NAME}' -ErrorAction Stop; $port = $rule | Get-NetFirewallPortFilter; $addr = $rule | Get-NetFirewallAddressFilter; Write-Output ([string]$rule.DisplayName); Write-Output ([string]$port.Protocol); Write-Output ([string]$port.LocalPort); Write-Output ([string]$addr.RemoteAddress); Write-Output ([string]$rule.Direction); Write-Output ([string]$rule.Profile); Write-Output ([string]$rule.Action); Write-Output ([string]$rule.Enabled)"#
                ),
            ])
            .output()
            .map_err(|error| DoctorQueryError::Invoke {
                operation: "get_firewall_rule",
                message: error.to_string(),
            })?;
        if !output.status.success() {
            return Err(DoctorQueryError::Invoke {
                operation: "get_firewall_rule",
                message: command_output_message(&output),
            });
        }
        parse_firewall_signature(&String::from_utf8_lossy(&output.stdout))
    }
}

fn command_output_message(output: &Output) -> String {
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
    if !stderr.is_empty() {
        return stderr;
    }
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    if !stdout.is_empty() {
        return stdout;
    }
    output.status.to_string()
}

pub(crate) fn parse_firewall_signature(
    output: &str,
) -> Result<FirewallSignature, DoctorQueryError> {
    let mut values = output
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty());
    let mut next_field = |name: &'static str| -> Result<String, DoctorQueryError> {
        values
            .next()
            .map(str::to_owned)
            .ok_or_else(|| DoctorQueryError::Malformed {
                operation: "get_firewall_rule",
                message: format!("missing {name} value"),
            })
    };
    Ok(FirewallSignature {
        display_name: next_field("DisplayName")?,
        protocol: next_field("Protocol")?,
        local_port: next_field("LocalPort")?,
        remote_address: next_field("RemoteAddress")?,
        direction: next_field("Direction")?,
        profile: next_field("Profile")?,
        action: next_field("Action")?,
        enabled: next_field("Enabled")?,
    })
}
