use std::io::Write;
use std::process::Command;

#[test]
fn emits_cpp_header_with_expected_constexpr() {
    let toml = br#"
[plugin]
name = "test"
version = "0.1.0"
type = "guest"

[runtime]
state = "ephemeral"
execution = "exclusive"
port = 50123
log_filter = "debug"

[runtime.paths]
sample_dir = "/opt/malbox/samples"
artifact_dir = "/opt/malbox/artifacts"
"#;
    let tmp = tempfile::tempdir().unwrap();
    let manifest_path = tmp.path().join("plugin.toml");
    let mut f = std::fs::File::create(&manifest_path).unwrap();
    f.write_all(toml).unwrap();

    let output_path = tmp.path().join("out.hpp");

    let status = Command::new(env!("CARGO_BIN_EXE_malbox-codegen"))
        .args([
            "--manifest",
            manifest_path.to_str().unwrap(),
            "--lang",
            "cpp",
            "--output",
            output_path.to_str().unwrap(),
        ])
        .status()
        .unwrap();
    assert!(status.success());

    let header = std::fs::read_to_string(&output_path).unwrap();
    assert!(
        header.contains(".port                   = 50123"),
        "got: {header}"
    );
    assert!(
        header.contains(r#".sample_dir             = "/opt/malbox/samples""#),
        "got: {header}"
    );
    assert!(
        header.contains(r#".artifact_dir           = "/opt/malbox/artifacts""#),
        "got: {header}"
    );
    assert!(
        header.contains(r#".log_filter             = "debug""#),
        "got: {header}"
    );
}

#[test]
fn emits_default_paths_when_not_specified() {
    let toml = br#"
[plugin]
name = "test2"
version = "0.1.0"
type = "guest"

[runtime]
state = "ephemeral"
execution = "exclusive"
port = 50200
"#;
    let tmp = tempfile::tempdir().unwrap();
    let manifest_path = tmp.path().join("plugin.toml");
    let mut f = std::fs::File::create(&manifest_path).unwrap();
    f.write_all(toml).unwrap();

    let output_path = tmp.path().join("out.hpp");

    let status = Command::new(env!("CARGO_BIN_EXE_malbox-codegen"))
        .args([
            "--manifest",
            manifest_path.to_str().unwrap(),
            "--lang",
            "cpp",
            "--output",
            output_path.to_str().unwrap(),
        ])
        .status()
        .unwrap();
    assert!(status.success());

    let header = std::fs::read_to_string(&output_path).unwrap();
    assert!(
        header.contains(r#".sample_dir             = "/tmp/malbox/samples""#),
        "expected default sample_dir; got: {header}"
    );
    assert!(
        header.contains(r#".stash_dir              = "/tmp/malbox/stash""#),
        "expected default stash_dir; got: {header}"
    );
}

#[test]
fn fails_on_invalid_runtime_section() {
    let toml = br#"
[plugin]
name = "test3"
version = "0.1.0"
type = "guest"

[runtime]
state = "ephemeral"
execution = "exclusive"
port = 80
"#;
    let tmp = tempfile::tempdir().unwrap();
    let manifest_path = tmp.path().join("plugin.toml");
    let mut f = std::fs::File::create(&manifest_path).unwrap();
    f.write_all(toml).unwrap();

    let output_path = tmp.path().join("out.hpp");

    let output = Command::new(env!("CARGO_BIN_EXE_malbox-codegen"))
        .args([
            "--manifest",
            manifest_path.to_str().unwrap(),
            "--lang",
            "cpp",
            "--output",
            output_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(!output.status.success(), "expected failure on invalid port");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("port"), "stderr: {stderr}");
    assert!(
        !output_path.exists(),
        "output file must not be created on error"
    );
}
