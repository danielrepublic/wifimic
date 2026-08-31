use wifimic_update::{
    parse_update_target, resolve_action, ResolvedAction, UpdateError, UpdateTarget,
};

#[test]
fn parse_update_target_uses_latest_when_omitted() {
    // Given
    let text = None;

    // When
    let result = parse_update_target(text);

    // Then
    assert_eq!(
        result.expect("omitted target is valid"),
        UpdateTarget::Latest
    );
}

#[test]
fn parse_update_target_uses_latest_keyword() {
    // Given
    let text = Some("latest");

    // When
    let result = parse_update_target(text);

    // Then
    assert_eq!(
        result.expect("latest target is valid"),
        UpdateTarget::Latest
    );
}

#[test]
fn parse_update_target_preserves_explicit_release_tag() {
    // Given
    let text = Some("v1.2.3");

    // When
    let result = parse_update_target(text);

    // Then
    assert_eq!(
        result.expect("release tag target is valid"),
        UpdateTarget::Tag("v1.2.3".to_owned())
    );
}

#[test]
fn parse_update_target_rejects_malformed_tag() {
    // Given
    let text = Some("not-a-tag");

    // When
    let result = parse_update_target(text);

    // Then
    assert_eq!(
        result,
        Err(UpdateError::InvalidTag {
            tag: "not-a-tag".to_owned(),
        })
    );
}

fn assert_resolution(target: UpdateTarget, current: &str, expected: ResolvedAction) {
    // Given
    let discovery = || Ok("v1.2.3".to_owned());

    // When
    let result = resolve_action(&target, current, discovery);

    // Then
    assert_eq!(result.expect("target resolves"), expected);
}

#[test]
fn resolve_action_follows_the_latest_and_explicit_tag_matrix() {
    assert_resolution(
        UpdateTarget::Latest,
        "v1.2.3",
        ResolvedAction::NoOp {
            current: "v1.2.3".to_owned(),
            latest: "v1.2.3".to_owned(),
        },
    );
    assert_resolution(
        UpdateTarget::Latest,
        "v1.2.2",
        ResolvedAction::Proceed {
            target_tag: "v1.2.3".to_owned(),
        },
    );
    assert_resolution(
        UpdateTarget::Latest,
        "v1.2.4",
        ResolvedAction::Proceed {
            target_tag: "v1.2.3".to_owned(),
        },
    );
    assert_resolution(
        UpdateTarget::Tag("v1.2.3".to_owned()),
        "v1.2.3",
        ResolvedAction::NoOp {
            current: "v1.2.3".to_owned(),
            latest: "v1.2.3".to_owned(),
        },
    );
    assert_resolution(
        UpdateTarget::Tag("v1.2.2".to_owned()),
        "v1.2.3",
        ResolvedAction::Proceed {
            target_tag: "v1.2.2".to_owned(),
        },
    );
    assert_resolution(
        UpdateTarget::Tag("v1.2.4".to_owned()),
        "v1.2.3",
        ResolvedAction::Proceed {
            target_tag: "v1.2.4".to_owned(),
        },
    );
}

#[test]
fn resolve_action_rejects_an_indeterminate_current_version_for_each_target_kind() {
    // Given
    let latest = UpdateTarget::Latest;
    let explicit = UpdateTarget::Tag("v1.2.4".to_owned());

    // When
    let latest_result = resolve_action(&latest, "v1.2.3-dev", || Ok("v1.2.4".to_owned()));
    let explicit_result = resolve_action(&explicit, "v1.2.3-dev", || {
        panic!("explicit target does not discover")
    });

    // Then
    let expected = Err(UpdateError::IndeterminateVersion {
        current: "v1.2.3-dev".to_owned(),
    });
    assert_eq!(latest_result, expected.clone());
    assert_eq!(explicit_result, expected);
}
