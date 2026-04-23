use std::io::Write;
use std::process::Command;

#[test]
fn emits_cpp_header_with_expected_constexpr() {
    let toml = br#"
[plugin]
name = "test"
version = "0.1.0"
type = "guest"
state = "ephemeral"
execution = "exclusive"

[runtime]
port = 50123
work_dir = "/opt/malbox"
log_filter = "debug"
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
        header.contains(r#".work_dir               = "/opt/malbox""#),
        "got: {header}"
    );
    assert!(
        header.contains(r#".log_filter             = "debug""#),
        "got: {header}"
    );
    assert!(
        header.contains(".log_overflow_dir       = nullptr"),
        "got: {header}"
    );
}

#[test]
fn emits_explicit_log_overflow_dir_when_set() {
    let toml = br#"
[plugin]
name = "test2"
version = "0.1.0"
type = "guest"
state = "ephemeral"
execution = "exclusive"

[runtime]
port = 50200
work_dir = "/opt/malbox"
log_overflow_dir = "/var/log/malbox"
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
        header.contains(r#".log_overflow_dir       = "/var/log/malbox""#),
        "got: {header}"
    );
    assert!(
        !header.contains("nullptr"),
        "nullptr should not appear when log_overflow_dir is explicit; got: {header}"
    );
}

#[test]
fn fails_on_invalid_runtime_section() {
    let toml = br#"
[plugin]
name = "test3"
version = "0.1.0"
type = "guest"
state = "ephemeral"
execution = "exclusive"

[runtime]
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
