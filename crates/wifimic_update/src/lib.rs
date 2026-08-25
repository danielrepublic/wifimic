use std::time::Duration;

const GITHUB_OWNER: &str = "danielrepublic";
const GITHUB_REPOSITORY: &str = "wifimic";
const RELEASE_MARKER: &str = "/releases/tag/";
const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);
const USER_AGENT: &str = "wifimic_server/manual-updater";

/// Describes the ordering of two release versions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VersionComparison {
    /// The two valid versions are equal.
    UpToDate,
    /// The latest version is newer than the current version.
    UpdateAvailable,
    /// The current version is newer than the latest version.
    CurrentNewer,
    /// One or both values could not be parsed as a `vMAJOR.MINOR.PATCH` version.
    Indeterminate,
}

/// Reports failures while discovering or comparing release versions.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum UpdateError {
    /// The HTTP request failed before a usable response was received.
    #[error("GitHub release request failed: {message}")]
    Network { message: String },
    /// GitHub returned a response other than the expected redirect.
    #[error("GitHub latest release returned unexpected HTTP status {status}")]
    UnexpectedStatus { status: u16 },
    /// The redirect response did not contain a Location header.
    #[error("GitHub latest release response did not contain a Location header")]
    MissingLocation,
    /// The Location header did not contain a valid release tag path.
    #[error("GitHub release Location header is not a valid release URL: {location:?}")]
    InvalidLocation { location: String },
    /// A discovered or explicitly supplied tag did not match the release-tag grammar.
    #[error("release tag {tag:?} is not vMAJOR.MINOR.PATCH")]
    InvalidTag { tag: String },
    /// A response body could not be read.
    #[error("GitHub release response body could not be read: {message}")]
    BodyRead { message: String },
    /// The current or latest value was not a stable release version.
    #[error("cannot determine a stable version comparison for {current:?}; use --tag explicitly")]
    IndeterminateVersion { current: String },
}

/// Parses a GitHub release redirect URL into its strict `vMAJOR.MINOR.PATCH` tag.
///
/// # Errors
/// Returns [`UpdateError::InvalidLocation`] when the URL has no release-tag suffix
/// or contains a query, fragment, slash, or empty tag. Returns
/// [`UpdateError::InvalidTag`] when the suffix is not a `vMAJOR.MINOR.PATCH` tag.
pub fn parse_release_tag(location: &str) -> Result<String, UpdateError> {
    let Some((_, tag)) = location.rsplit_once(RELEASE_MARKER) else {
        return Err(UpdateError::InvalidLocation {
            location: location.to_owned(),
        });
    };
    if tag.is_empty() || tag.contains('/') || tag.contains('?') || tag.contains('#') {
        return Err(UpdateError::InvalidLocation {
            location: location.to_owned(),
        });
    }
    if !is_release_tag(tag) {
        return Err(UpdateError::InvalidTag {
            tag: tag.to_owned(),
        });
    }
    Ok(tag.to_owned())
}

/// Returns whether a string has the strict `vMAJOR.MINOR.PATCH` release-tag shape.
#[must_use]
pub fn is_release_tag(tag: &str) -> bool {
    let Some(version) = tag.strip_prefix('v') else {
        return false;
    };
    let mut components = version.split('.');
    let Some(major) = components.next() else {
        return false;
    };
    let Some(minor) = components.next() else {
        return false;
    };
    let Some(patch) = components.next() else {
        return false;
    };
    components.next().is_none()
        && !major.is_empty()
        && !minor.is_empty()
        && !patch.is_empty()
        && [major, minor, patch]
            .iter()
            .all(|component| component.bytes().all(|byte| byte.is_ascii_digit()))
}

/// Compares two embedded or discovered `vMAJOR.MINOR.PATCH` version strings.
#[must_use]
pub fn compare_versions(current: &str, latest: &str) -> VersionComparison {
    let Some(current_version) = parse_semver(current) else {
        return VersionComparison::Indeterminate;
    };
    let Some(latest_version) = parse_semver(latest) else {
        return VersionComparison::Indeterminate;
    };
    match current_version.cmp(&latest_version) {
        std::cmp::Ordering::Less => VersionComparison::UpdateAvailable,
        std::cmp::Ordering::Equal => VersionComparison::UpToDate,
        std::cmp::Ordering::Greater => VersionComparison::CurrentNewer,
    }
}

fn parse_semver(value: &str) -> Option<semver::Version> {
    is_release_tag(value)
        .then(|| value.strip_prefix('v'))
        .flatten()
        .and_then(|version| semver::Version::parse(version).ok())
}

/// Discovers the latest public GitHub release tag without following the redirect.
///
/// # Errors
/// Returns a typed error when the request times out, GitHub returns an unexpected
/// status, the Location header is missing or malformed, or the tag is invalid.
pub fn discover_latest_tag() -> Result<String, UpdateError> {
    let url = format!("https://github.com/{GITHUB_OWNER}/{GITHUB_REPOSITORY}/releases/latest");
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .max_redirects(0)
        .user_agent(USER_AGENT)
        .timeout_global(Some(REQUEST_TIMEOUT))
        .http_status_as_error(false)
        .build()
        .into();
    let response = agent
        .get(&url)
        .call()
        .map_err(|error| UpdateError::Network {
            message: error.to_string(),
        })?;
    if response.status().as_u16() != 302 {
        return Err(UpdateError::UnexpectedStatus {
            status: response.status().as_u16(),
        });
    }
    let Some(location) = response
        .headers()
        .get("Location")
        .and_then(|value| value.to_str().ok())
    else {
        return Err(UpdateError::MissingLocation);
    };
    parse_release_tag(location)
}

/// Downloads one public GitHub release asset into memory for a caller to verify.
///
/// # Errors
/// Returns a typed error when the request or response-body read fails.
pub fn download_release_asset(tag: &str, asset: &str) -> Result<Vec<u8>, UpdateError> {
    if !is_release_tag(tag) {
        return Err(UpdateError::InvalidTag {
            tag: tag.to_owned(),
        });
    }
    let url = format!(
        "https://github.com/{GITHUB_OWNER}/{GITHUB_REPOSITORY}/releases/download/{tag}/{asset}"
    );
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .user_agent(USER_AGENT)
        .timeout_global(Some(REQUEST_TIMEOUT))
        .build()
        .into();
    let mut response = agent
        .get(&url)
        .call()
        .map_err(|error| UpdateError::Network {
            message: error.to_string(),
        })?;
    response
        .body_mut()
        .read_to_vec()
        .map_err(|error| UpdateError::BodyRead {
            message: error.to_string(),
        })
}

#[cfg(test)]
mod tests {
    use super::{compare_versions, is_release_tag, parse_release_tag, VersionComparison};

    #[test]
    fn compares_equal_versions() {
        // Given
        let current = "v0.1.12";
        let latest = "v0.1.12";

        // When
        let result = compare_versions(current, latest);

        // Then
        assert_eq!(result, VersionComparison::UpToDate);
    }

    #[test]
    fn compares_current_older_than_latest() {
        // Given
        let current = "v0.1.11";
        let latest = "v0.1.12";

        // When
        let result = compare_versions(current, latest);

        // Then
        assert_eq!(result, VersionComparison::UpdateAvailable);
    }

    #[test]
    fn compares_current_newer_than_latest() {
        // Given
        let current = "v1.0.0";
        let latest = "v0.99.99";

        // When
        let result = compare_versions(current, latest);

        // Then
        assert_eq!(result, VersionComparison::CurrentNewer);
    }

    #[test]
    fn malformed_current_version_is_indeterminate() {
        // Given
        let current = "v0.1.0-dev";

        // When
        let result = compare_versions(current, "v0.1.1");

        // Then
        assert_eq!(result, VersionComparison::Indeterminate);
    }

    #[test]
    fn malformed_latest_tag_is_indeterminate() {
        // Given
        let latest = "release-0.1.1";

        // When
        let result = compare_versions("v0.1.0", latest);

        // Then
        assert_eq!(result, VersionComparison::Indeterminate);
    }

    #[test]
    fn parses_valid_location_header() {
        // Given
        let location = "https://github.com/danielrepublic/wifimic/releases/tag/v0.1.12";

        // When
        let result = parse_release_tag(location);

        // Then
        assert_eq!(result.expect("valid fabricated Location"), "v0.1.12");
    }

    #[test]
    fn rejects_malformed_location_headers() {
        // Given
        let locations = [
            "https://github.com/danielrepublic/wifimic/releases/latest",
            "",
            "https://github.com/danielrepublic/wifimic/releases/tag/0.1.12",
            "https://github.com/danielrepublic/wifimic/releases/tag/v0.1.12/",
            "https://github.com/danielrepublic/wifimic/releases/tag/v0.1.12?x=1",
        ];

        // When
        let mut results = locations.iter().map(|location| parse_release_tag(location));

        // Then
        assert!(results.all(|result| result.is_err()));
    }

    #[test]
    fn recognizes_only_numeric_three_component_release_tags() {
        // Given
        let valid = "v9.9.9";
        let invalid = ["9.9.9", "v9.9", "v9.9.9-beta", "v9.a.9"];

        // When
        let valid_result = is_release_tag(valid);
        let mut invalid_results = invalid.iter().map(|tag| is_release_tag(tag));

        // Then
        assert!(valid_result);
        assert!(invalid_results.all(|result| !result));
    }
}
