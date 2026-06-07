//! Guards the agreement between the feature table in malbox-installer and
//! this crate's Cargo feature declarations. The release workflow builds the
//! prebuilt binaries with the Cargo `default` set, and the installer decides
//! prebuilt eligibility (and replays from-source upgrades) from the table,
//! so drift between them silently breaks those decisions.

fn cargo_toml() -> toml::Value {
    toml::from_str(include_str!("../Cargo.toml")).expect("malboxctl Cargo.toml parses")
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
         bin/malboxctl/Cargo.toml (release binaries are built with Cargo defaults)"
    );
}

#[test]
fn table_features_exist_in_cargo() {
    let cargo = cargo_toml();
    let features = cargo["features"].as_table().expect("[features] is a table");

    for entry in malbox_installer::DAEMON_FEATURES {
        assert!(
            features.contains_key(entry.feature),
            "feature '{}' from the installer table is not declared in bin/malboxctl/Cargo.toml",
            entry.feature
        );
    }
}
