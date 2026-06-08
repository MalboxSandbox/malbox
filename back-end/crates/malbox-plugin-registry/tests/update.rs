use malbox_plugin_registry::update::needs_update;

#[test]
fn newer_version_needs_update() {
    assert!(needs_update("0.1.0", "0.2.0"));
}

#[test]
fn same_version_no_update() {
    assert!(!needs_update("0.1.0", "0.1.0"));
}

#[test]
fn older_version_no_update() {
    assert!(!needs_update("0.2.0", "0.1.0"));
}

#[test]
fn prerelease_to_release_needs_update() {
    assert!(needs_update("0.1.0-alpha.1", "0.1.0"));
}

#[test]
fn invalid_version_no_update() {
    assert!(!needs_update("not-semver", "0.1.0"));
}
