use malbox_plugin_registry::source::{BuildSystem, check_tool, detect_build_system};
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
