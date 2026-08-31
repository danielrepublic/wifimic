use wifimic_update::{compare_versions, discover_latest_tag, UpdateError, VersionComparison};

/// Provides one latest-release lookup for the manual update check.
pub(crate) trait TagDiscovery {
    /// Returns the latest public release tag.
    fn latest_tag(&self) -> Result<String, UpdateError>;
}

/// Uses GitHub's latest-release redirect for the manual update check.
#[derive(Debug, Default)]
pub(crate) struct NativeTagDiscovery;

impl TagDiscovery for NativeTagDiscovery {
    fn latest_tag(&self) -> Result<String, UpdateError> {
        discover_latest_tag()
    }
}

/// Represents a successful, non-mutating update check.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum CheckUpdateOutcome {
    /// The current and latest release tags are equal.
    UpToDate { current: String, latest: String },
    /// A newer release is available.
    UpdateAvailable { current: String, latest: String },
    /// The current binary is newer than the public latest release.
    CurrentNewer { current: String, latest: String },
}

impl CheckUpdateOutcome {
    pub(crate) fn render(&self) -> String {
        match self {
            Self::UpToDate { current, .. } => format!("目前版本 {current} 已是最新版本"),
            Self::UpdateAvailable { current, latest } => format!(
                "有新版本可用：{current} → {latest}，執行 `wifimic_server upgrade` 進行更新"
            ),
            Self::CurrentNewer { current, latest } => {
                format!("目前版本 {current} 比最新版本 {latest} 更新")
            }
        }
    }
}

/// Renders the one-line output for a check result.
pub(crate) fn render_check_update(result: &Result<CheckUpdateOutcome, UpdateError>) -> String {
    match result {
        Ok(outcome) => outcome.render(),
        Err(error) => format!("更新檢查失敗：{error}"),
    }
}

/// Returns the process exit code for a check result.
#[must_use]
pub(crate) fn check_update_exit_code(result: &Result<CheckUpdateOutcome, UpdateError>) -> u8 {
    if result.is_ok() {
        0
    } else {
        1
    }
}

/// Runs a one-shot check without downloading or mutating anything.
///
/// # Errors
/// Returns [`UpdateError`] when the latest tag cannot be discovered or the
/// embedded current version cannot be compared with it.
pub(crate) fn run_check_update<D: TagDiscovery>(
    discovery: &D,
    current: &str,
) -> Result<CheckUpdateOutcome, UpdateError> {
    let latest = discovery.latest_tag()?;
    let outcome = match compare_versions(current, &latest) {
        VersionComparison::UpToDate => CheckUpdateOutcome::UpToDate {
            current: current.to_owned(),
            latest,
        },
        VersionComparison::UpdateAvailable => CheckUpdateOutcome::UpdateAvailable {
            current: current.to_owned(),
            latest,
        },
        VersionComparison::CurrentNewer => CheckUpdateOutcome::CurrentNewer {
            current: current.to_owned(),
            latest,
        },
        VersionComparison::Indeterminate => {
            return Err(UpdateError::IndeterminateVersion {
                current: current.to_owned(),
            })
        }
    };
    Ok(outcome)
}

#[cfg(test)]
mod tests {
    use super::{
        check_update_exit_code, render_check_update, run_check_update, CheckUpdateOutcome,
        TagDiscovery,
    };
    use wifimic_update::UpdateError;

    #[derive(Debug)]
    struct FakeDiscovery {
        result: Result<String, UpdateError>,
    }

    impl TagDiscovery for FakeDiscovery {
        fn latest_tag(&self) -> Result<String, UpdateError> {
            match &self.result {
                Ok(tag) => Ok(tag.clone()),
                Err(error) => Err(match error {
                    UpdateError::Network { message } => UpdateError::Network {
                        message: message.clone(),
                    },
                    UpdateError::UnexpectedStatus { status } => {
                        UpdateError::UnexpectedStatus { status: *status }
                    }
                    UpdateError::MissingLocation => UpdateError::MissingLocation,
                    UpdateError::InvalidLocation { location } => UpdateError::InvalidLocation {
                        location: location.clone(),
                    },
                    UpdateError::InvalidTag { tag } => UpdateError::InvalidTag { tag: tag.clone() },
                    UpdateError::BodyRead { message } => UpdateError::BodyRead {
                        message: message.clone(),
                    },
                    UpdateError::IndeterminateVersion { current } => {
                        UpdateError::IndeterminateVersion {
                            current: current.clone(),
                        }
                    }
                    UpdateError::InvalidChecksumManifest => UpdateError::InvalidChecksumManifest,
                    UpdateError::ChecksumMismatch { expected, actual } => {
                        UpdateError::ChecksumMismatch {
                            expected: expected.clone(),
                            actual: actual.clone(),
                        }
                    }
                }),
            }
        }
    }

    #[test]
    fn check_update_reports_up_to_date_without_failure() {
        // Given
        let discovery = FakeDiscovery {
            result: Ok("v0.1.12".to_owned()),
        };

        // When
        let result = run_check_update(&discovery, "v0.1.12").expect("fake check succeeds");

        // Then
        assert_eq!(
            result,
            CheckUpdateOutcome::UpToDate {
                current: "v0.1.12".to_owned(),
                latest: "v0.1.12".to_owned(),
            }
        );
        assert_eq!(check_update_exit_code(&Ok(result)), 0);
    }

    #[test]
    fn check_update_reports_available_release_without_failure() {
        // Given
        let discovery = FakeDiscovery {
            result: Ok("v0.2.0".to_owned()),
        };

        // When
        let result = run_check_update(&discovery, "v0.1.12").expect("fake check succeeds");

        // Then
        assert!(matches!(
            &result,
            CheckUpdateOutcome::UpdateAvailable { .. }
        ));
        assert!(render_check_update(&Ok(result)).contains("有新版本可用"));
    }

    #[test]
    fn check_update_returns_network_failure_for_nonzero_exit_path() {
        // Given
        let discovery = FakeDiscovery {
            result: Err(UpdateError::Network {
                message: "offline".to_owned(),
            }),
        };

        // When
        let result = run_check_update(&discovery, "v0.1.12");

        // Then
        assert_eq!(check_update_exit_code(&result), 1);
        assert!(render_check_update(&result).contains("更新檢查失敗"));
        assert!(matches!(result, Err(UpdateError::Network { .. })));
    }
}
