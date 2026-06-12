//! Validates the build.rs-generated feature table against the actual Cargo
//! feature declarations. The build.rs reads this same Cargo.toml at compile
//! time, so these tests are a belt-and-suspenders correctness check on the
//! code generation.

fn cargo_toml() -> toml::Value {
    toml::from_str(include_str!("../Cargo.toml")).expect("malboxd Cargo.toml parses")
}

#[test]
fn table_defaults_match_cargo_defaults() {
    let cargo = cargo_toml();
    let mut cargo_defaults: Vec<String> = cargo["features"]["default"]
        .as_array()
        .expect("[features] default is an array")
        .iter()
        .map(|v| v.as_str().expect("feature name").to_string())
        .collect();
    cargo_defaults.sort_unstable();

    let mut table_defaults = malbox_installer::default_features();
    table_defaults.sort_unstable();

    assert_eq!(
        table_defaults, cargo_defaults,
        "DAEMON_FEATURES defaults must mirror the [features] default set in \
         bin/malboxd/Cargo.toml (release binaries are built with Cargo defaults)"
    );
}

#[test]
fn table_features_exist_in_cargo() {
    let cargo = cargo_toml();
    let features = cargo["features"].as_table().expect("[features] is a table");

    for entry in malbox_installer::DAEMON_FEATURES {
        assert!(
            features.contains_key(entry.feature),
            "feature '{}' from the installer table is not declared in bin/malboxd/Cargo.toml",
            entry.feature
        );
    }
}

#[test]
fn table_features_have_correct_kind() {
    for entry in malbox_installer::DAEMON_FEATURES {
        let expected = if entry.feature.starts_with("provider-") {
            malbox_installer::FeatureKind::Provider
        } else if entry.feature.starts_with("provisioner-") {
            malbox_installer::FeatureKind::Provisioner
        } else {
            panic!(
                "feature '{}' has no recognized prefix (expected provider-* or provisioner-*)",
                entry.feature
            );
        };
        assert_eq!(
            entry.kind, expected,
            "feature '{}' has wrong FeatureKind",
            entry.feature
        );
    }
}
