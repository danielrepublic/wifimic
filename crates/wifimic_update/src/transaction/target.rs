use crate::{compare_versions, is_release_tag, UpdateError, VersionComparison};

/// Selects either the authoritative latest release or a deliberate release tag.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpdateTarget {
    /// Resolve and install the latest public release.
    Latest,
    /// Install the supplied stable release tag.
    Tag(String),
}

/// Parses a user-selected update target at the command boundary.
///
/// # Errors
///
/// Returns [`UpdateError::InvalidTag`] when the target is neither `latest` nor
/// a strict `vMAJOR.MINOR.PATCH` release tag.
pub fn parse_update_target(text: Option<&str>) -> Result<UpdateTarget, UpdateError> {
    match text {
        None | Some("latest") => Ok(UpdateTarget::Latest),
        Some(tag) if is_release_tag(tag) => Ok(UpdateTarget::Tag(tag.to_owned())),
        Some(tag) => Err(UpdateError::InvalidTag {
            tag: tag.to_owned(),
        }),
    }
}

/// Chooses whether an already-resolved target requires an update transaction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolvedAction {
    /// The selected target already matches the installed version.
    NoOp { current: String, latest: String },
    /// Callers may begin their update transaction for this target tag.
    Proceed { target_tag: String },
}

/// Resolves an update target without mutating installed or managed state.
///
/// Callers provide only latest-tag discovery and must not mutate before a
/// [`ResolvedAction::Proceed`] result.
///
/// # Errors
///
/// Returns [`UpdateError::InvalidTag`] when discovery or an explicit target
/// does not produce a strict release tag. Returns
/// [`UpdateError::IndeterminateVersion`] when the current version cannot be
/// compared to the selected target.
pub fn resolve_action<F>(
    target: &UpdateTarget,
    current: &str,
    discover_latest: F,
) -> Result<ResolvedAction, UpdateError>
where
    F: FnOnce() -> Result<String, UpdateError>,
{
    let target_tag = match target {
        UpdateTarget::Latest => discover_latest()?,
        UpdateTarget::Tag(tag) => tag.clone(),
    };
    if !is_release_tag(&target_tag) {
        return Err(UpdateError::InvalidTag { tag: target_tag });
    }
    match compare_versions(current, &target_tag) {
        VersionComparison::UpToDate => Ok(ResolvedAction::NoOp {
            current: current.to_owned(),
            latest: target_tag,
        }),
        VersionComparison::UpdateAvailable | VersionComparison::CurrentNewer => {
            Ok(ResolvedAction::Proceed { target_tag })
        }
        VersionComparison::Indeterminate => Err(UpdateError::IndeterminateVersion {
            current: current.to_owned(),
        }),
    }
}
