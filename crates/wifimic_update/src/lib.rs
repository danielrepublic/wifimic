//! The shared Update Contract for wifimic.
//!
//! This crate is the single deep module that owns the update transaction both
//! platforms follow. It resolves an [`UpdateTarget`], verifies a release
//! artifact's fingerprint, and runs the fixed
//! `backup → pre_swap → swap → post_swap → health_check → rollback` order
//! through the narrow [`UpdateAdapter`] trait. Adapters own only platform
//! mechanics: archive handling, process lifecycle, and health-check details.
//! The transaction order, the rollback semantics, and the outcome/error types
//! live here and nowhere else.
//!
//! The contract is defined in `CONTEXT.md` ("更新合約 (Update Contract)") and
//! its intent is recorded in
//! [`docs/adr/0001-windows-update-moves-from-source-build-to-self-updater-binary.md`](../../../docs/adr/0001-windows-update-moves-from-source-build-to-self-updater-binary.md).
//!
//! # Windows handoff
//!
//! On Windows, `wifimic_client upgrade` cannot replace a running executable,
//! so it elevates a thin handoff script that performs the transaction by
//! invoking **a temporary runner copy** of `wifimic_client.exe` with the
//! hidden `--internal-apply-upgrade <tag>` entry point. The runner is copied
//! from the canonical install path to a unique temporary path and deleted
//! after use; the canonical path is never invoked directly, because a process
//! must not replace its own currently-executing image. The transaction logic
//! is **not** reimplemented in PowerShell: the handoff script only waits for
//! the parent process to exit and relays the validated tag to the runner, so
//! 100% of the transaction stays in this already-testable Rust engine. This
//! design choice is recorded in the plan's Decision 4 and todo 16's
//! round-13 self-replacement fix.

use std::time::Duration;

use sha2::{Digest, Sha256};

pub mod check;
pub mod transaction;

// allow: SIZE_OK — root API owns shared update definitions and public re-exports.
pub use check::{
    check_update_exit_code, render_check_update, run_check_update, CheckUpdateOutcome,
};

pub use transaction::{
    parse_update_target, resolve_action, run_update_transaction, ResolvedAction, RollbackOutcome,
    TransactionError, TransactionOutcome, UpdateAdapter, UpdateTarget,
};

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
#[derive(Debug, Clone, thiserror::Error, PartialEq, Eq)]
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
    #[error(
        "cannot determine a stable version comparison for {current:?}; use an explicit vMAJOR.MINOR.PATCH target"
    )]
    IndeterminateVersion { current: String },
    /// The checksum manifest is malformed or does not contain a valid SHA-256 digest.
    #[error("release checksum manifest is malformed")]
    InvalidChecksumManifest,
    /// The downloaded archive did not match its published checksum digest.
    #[error("release checksum mismatch: expected {expected}, got {actual}")]
    ChecksumMismatch { expected: String, actual: String },
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

/// Verifies a release artifact's checksum against a manifest.
///
/// Parses the manifest bytes for a 64-hex lowercase SHA-256 digest and compares
/// it against the SHA-256 of the archive bytes.
///
/// # Errors
/// Returns [`UpdateError::InvalidChecksumManifest`] if the manifest does not
/// contain a valid 64-hex lowercase digest. Returns
/// [`UpdateError::ChecksumMismatch`] if the computed digest does not match.
pub fn verify_release_fingerprint(
    archive_bytes: &[u8],
    manifest_bytes: &[u8],
) -> Result<(), UpdateError> {
    let expected = std::str::from_utf8(manifest_bytes)
        .ok()
        .and_then(|value| value.split_whitespace().next())
        .filter(|value| value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit()))
        .map(str::to_ascii_lowercase)
        .ok_or(UpdateError::InvalidChecksumManifest)?;
    let actual = format!("{:x}", Sha256::digest(archive_bytes));
    if expected != actual {
        return Err(UpdateError::ChecksumMismatch { expected, actual });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        compare_versions, is_release_tag, parse_release_tag, verify_release_fingerprint,
        UpdateError, VersionComparison,
    };
    use sha2::{Digest, Sha256};

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

    #[test]
    fn verify_release_fingerprint_happy_path() {
        // Given
        let digest = Sha256::digest(b"test archive data");
        let expected_hex = format!("{:x}", digest);
        let manifest = format!("{}  test archive data", expected_hex);

        // When
        let result = verify_release_fingerprint(b"test archive data", manifest.as_bytes());

        // Then
        assert!(result.is_ok());
    }

    #[test]
    fn verify_release_fingerprint_mismatched_digest() {
        // Given
        let archive = b"some archive data";
        let manifest = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

        // When
        let result = verify_release_fingerprint(archive, manifest.as_bytes());

        // Then
        assert!(matches!(result, Err(UpdateError::ChecksumMismatch { .. })));
    }

    #[test]
    fn verify_release_fingerprint_malformed_manifest() {
        // Given
        let archive = b"some archive data";
        let manifest = "this is not a valid manifest";

        // When
        let result = verify_release_fingerprint(archive, manifest.as_bytes());

        // Then
        assert!(matches!(result, Err(UpdateError::InvalidChecksumManifest)));
    }

    #[test]
    fn verify_release_fingerprint_uppercase_manifest_normalizes() {
        // Given
        let archive_bytes = b"test data";
        let expected_sha = Sha256::digest(archive_bytes);
        let expected_hex = format!("{:x}", expected_sha);
        let manifest = format!("{}  test data", expected_hex.to_ascii_uppercase());

        // When
        let result = verify_release_fingerprint(archive_bytes, manifest.as_bytes());

        // Then
        assert!(result.is_ok());
    }
}
