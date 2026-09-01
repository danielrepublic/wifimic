use crate::{compare_versions, UpdateError, VersionComparison};

/// Represents a successful, non-mutating update check.
#[derive(Debug, PartialEq, Eq)]
pub enum CheckUpdateOutcome {
    /// The current and latest release tags are equal.
    UpToDate { current: String, latest: String },
    /// A newer release is available.
    UpdateAvailable { current: String, latest: String },
    /// The current binary is newer than the public latest release.
    CurrentNewer { current: String, latest: String },
}

impl CheckUpdateOutcome {
    /// Renders this outcome for the supplied binary name.
    #[must_use]
    pub fn render(&self, binary_name: &str) -> String {
        match self {
            Self::UpToDate { current, .. } => format!("目前版本 {current} 已是最新版本"),
            Self::UpdateAvailable { current, latest } => {
                format!("有新版本可用：{current} → {latest}，執行 `{binary_name} upgrade` 進行更新")
            }
            Self::CurrentNewer { current, latest } => {
                format!("目前版本 {current} 比最新版本 {latest} 更新")
            }
        }
    }
}

/// Renders the one-line output for a check result.
#[must_use]
pub fn render_check_update(
    result: &Result<CheckUpdateOutcome, UpdateError>,
    binary_name: &str,
) -> String {
    match result {
        Ok(outcome) => outcome.render(binary_name),
        Err(error) => format!("更新檢查失敗：{error}"),
    }
}

/// Returns the process exit code for a check result.
#[must_use]
pub fn check_update_exit_code(result: &Result<CheckUpdateOutcome, UpdateError>) -> u8 {
    match result {
        Ok(_) => 0,
        Err(_) => 1,
    }
}

/// Runs a one-shot check without downloading or mutating anything.
///
/// # Errors
///
/// Returns [`UpdateError`] when the latest tag cannot be discovered or the
/// current version cannot be compared with it.
pub fn run_check_update(
    current: &str,
    discover_latest: impl FnOnce() -> Result<String, UpdateError>,
) -> Result<CheckUpdateOutcome, UpdateError> {
    let latest = discover_latest()?;
    match compare_versions(current, &latest) {
        VersionComparison::UpToDate => Ok(CheckUpdateOutcome::UpToDate {
            current: current.to_owned(),
            latest,
        }),
        VersionComparison::UpdateAvailable => Ok(CheckUpdateOutcome::UpdateAvailable {
            current: current.to_owned(),
            latest,
        }),
        VersionComparison::CurrentNewer => Ok(CheckUpdateOutcome::CurrentNewer {
            current: current.to_owned(),
            latest,
        }),
        VersionComparison::Indeterminate => Err(UpdateError::IndeterminateVersion {
            current: current.to_owned(),
        }),
    }
}

#[cfg(test)]
#[path = "check_tests.rs"]
mod tests;
