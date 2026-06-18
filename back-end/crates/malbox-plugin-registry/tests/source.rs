use malbox_installer::progress::{NullObserver, ProgressObserver};
use malbox_plugin_registry::source::{
    BuildSystem, check_tool, detect_build_system, parse_cargo_artifact, parse_cmake_progress,
};
use tempfile::TempDir;

#[test]
fn detect_cargo_project() {
    let tmp = TempDir::new().unwrap();
    std::fs::write(
        tmp.path().join("Cargo.toml"),
        "[package]\nname = \"test\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    )
    .unwrap();
    let bs = detect_build_system(tmp.path()).unwrap();
    assert!(matches!(bs, BuildSystem::Cargo));
}

#[test]
fn detect_cmake_project() {
    let tmp = TempDir::new().unwrap();
    std::fs::write(
        tmp.path().join("CMakeLists.txt"),
        "cmake_minimum_required(VERSION 3.20)\n",
    )
    .unwrap();
    let bs = detect_build_system(tmp.path()).unwrap();
    assert!(matches!(bs, BuildSystem::CMake));
}

#[test]
fn detect_python_setup_py() {
    let tmp = TempDir::new().unwrap();
    std::fs::write(
        tmp.path().join("setup.py"),
        "from setuptools import setup\n",
    )
    .unwrap();
    let bs = detect_build_system(tmp.path()).unwrap();
    assert!(matches!(bs, BuildSystem::Python));
}

#[test]
fn detect_python_pyproject() {
    let tmp = TempDir::new().unwrap();
    std::fs::write(tmp.path().join("pyproject.toml"), "[build-system]\n").unwrap();
    let bs = detect_build_system(tmp.path()).unwrap();
    assert!(matches!(bs, BuildSystem::Python));
}

#[test]
fn detect_cargo_takes_priority_over_cmake() {
    let tmp = TempDir::new().unwrap();
    std::fs::write(tmp.path().join("Cargo.toml"), "[package]\n").unwrap();
    std::fs::write(
        tmp.path().join("CMakeLists.txt"),
        "cmake_minimum_required(VERSION 3.20)\n",
    )
    .unwrap();
    let bs = detect_build_system(tmp.path()).unwrap();
    assert!(matches!(bs, BuildSystem::Cargo));
}

#[test]
fn detect_unknown_project_errors() {
    let tmp = TempDir::new().unwrap();
    std::fs::write(tmp.path().join("README.md"), "# Hello\n").unwrap();
    let result = detect_build_system(tmp.path());
    assert!(result.is_err());
}

#[test]
fn check_tool_git_exists() {
    assert!(check_tool("git").is_ok());
}

#[test]
fn check_tool_nonexistent_errors() {
    let result = check_tool("definitely-not-a-real-tool-xyz");
    assert!(result.is_err());
}

#[test]
fn null_observer_is_send_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<NullObserver>();
}

#[test]
fn null_observer_methods_are_callable() {
    let obs = NullObserver;
    obs.step_started("clone");
    obs.step_completed("clone", "1.4s");
    obs.step_failed("build", "compilation error");
    obs.build_progress(5, 20, "serde");
    obs.build_output("Compiling serde v1.0.203");
    obs.download_progress(1024, Some(2048), "asset");
}

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
