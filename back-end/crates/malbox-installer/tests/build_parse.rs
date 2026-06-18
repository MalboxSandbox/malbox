use malbox_installer::build_parse::{parse_cargo_artifact, parse_cmake_progress};

#[test]
fn parse_cargo_artifact_extracts_crate_name() {
    let json = r#"{"reason":"compiler-artifact","package_id":"serde 1.0.203 (registry+https://github.com/rust-lang/crates.io-index)","manifest_path":"/tmp/.cargo/registry/src/serde-1.0.203/Cargo.toml","target":{"name":"serde"},"profile":{"opt_level":"3"},"features":[],"filenames":[],"executable":null,"fresh":false}"#;
    let result = parse_cargo_artifact(json);
    assert_eq!(result, Some("serde".to_string()));
}

#[test]
fn parse_cargo_artifact_ignores_non_artifact() {
    let json = r#"{"reason":"build-script-executed","package_id":"serde 1.0.203"}"#;
    assert_eq!(parse_cargo_artifact(json), None);
}

#[test]
fn parse_cargo_artifact_ignores_garbage() {
    assert_eq!(parse_cargo_artifact("not json at all"), None);
}

#[test]
fn parse_cmake_progress_extracts_percentage() {
    assert_eq!(
        parse_cmake_progress("[  5%] Building CXX object foo.cpp.o"),
        Some(5)
    );
    assert_eq!(
        parse_cmake_progress("[ 50%] Building CXX object bar.cpp.o"),
        Some(50)
    );
    assert_eq!(
        parse_cmake_progress("[100%] Linking CXX executable app"),
        Some(100)
    );
}

#[test]
fn parse_cmake_progress_ignores_non_progress() {
    assert_eq!(parse_cmake_progress("-- Configuring done"), None);
    assert_eq!(
        parse_cmake_progress("Scanning dependencies of target foo"),
        None
    );
}
