use malbox_plugin_registry::index::Dependency;
use malbox_plugin_registry::install::{check_soft_deps, validate_extracted_plugin};
use malbox_plugin_registry::lockfile::Lockfile;
use std::collections::HashMap;
use std::os::unix::fs::PermissionsExt;
use tempfile::TempDir;

#[test]
fn validate_good_plugin_directory() {
    let tmp = TempDir::new().unwrap();
    let plugin_dir = tmp.path().join("test-plugin");
    std::fs::create_dir(&plugin_dir).unwrap();

    std::fs::write(
        plugin_dir.join("plugin.toml"),
        r#"
[plugin]
name = "test-plugin"
version = "0.1.0"
type = "host"
[runtime]
state = "persistent"
execution = "parallel"
"#,
    )
    .unwrap();

    let binary = plugin_dir.join("test-plugin");
    std::fs::write(&binary, "#!/bin/sh\n").unwrap();
    std::fs::set_permissions(&binary, std::fs::Permissions::from_mode(0o755)).unwrap();

    assert!(validate_extracted_plugin(&plugin_dir).is_ok());
}

#[test]
fn validate_missing_manifest() {
    let tmp = TempDir::new().unwrap();
    let plugin_dir = tmp.path().join("test-plugin");
    std::fs::create_dir(&plugin_dir).unwrap();

    let result = validate_extracted_plugin(&plugin_dir);
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("plugin.toml"), "error was: {err}");
}

#[test]
fn soft_deps_all_present() {
    let requires = vec![Dependency {
        name: "file-info".into(),
        version: ">=0.1.0".into(),
    }];

    let mut plugins = HashMap::new();
    plugins.insert(
        "file-info".into(),
        malbox_plugin_registry::lockfile::LockedPlugin {
            version: "0.1.0".into(),
            repository: "malbox-plugins/file-info".into(),
            source: malbox_plugin_registry::lockfile::InstallSource::Registry,
            install_method: malbox_plugin_registry::lockfile::InstallMethod::Prebuilt,
            asset: None,
            checksum: None,
            installed_at: "2026-06-07T12:00:00Z".into(),
            pin: malbox_plugin_registry::lockfile::PinKind::Release,
            commit: None,
            path: None,
        },
    );
    let lockfile = Lockfile {
        schema_version: 1,
        plugins,
    };

    let missing = check_soft_deps(&requires, &lockfile);
    assert!(missing.is_empty());
}

#[test]
fn soft_deps_some_missing() {
    let requires = vec![
        Dependency {
            name: "file-info".into(),
            version: ">=0.1.0".into(),
        },
        Dependency {
            name: "hash-extractor".into(),
            version: ">=0.2.0".into(),
        },
    ];

    let lockfile = Lockfile::empty();
    let missing = check_soft_deps(&requires, &lockfile);

    assert_eq!(missing.len(), 2);
    assert!(missing.iter().any(|d| d.name == "file-info"));
    assert!(missing.iter().any(|d| d.name == "hash-extractor"));
}
