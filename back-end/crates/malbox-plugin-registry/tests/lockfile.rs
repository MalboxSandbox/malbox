use malbox_plugin_registry::lockfile::{
    InstallMethod, InstallSource, LockedPlugin, Lockfile, PinKind,
};
use std::collections::HashMap;
use tempfile::TempDir;

fn sample_lockfile() -> Lockfile {
    let mut plugins = HashMap::new();
    plugins.insert(
        "yara-scanner".to_string(),
        LockedPlugin {
            version: "0.1.0".to_string(),
            repository: "malbox-plugins/yara-scanner".to_string(),
            source: InstallSource::Registry,
            install_method: InstallMethod::Prebuilt,
            asset: Some("yara-scanner-v0.1.0-x86_64-linux.tar.gz".to_string()),
            checksum: Some("sha256:abc123".to_string()),
            installed_at: "2026-06-07T12:00:00Z".to_string(),
            pin: PinKind::Release,
            commit: None,
            path: None,
        },
    );
    Lockfile {
        schema_version: 1,
        plugins,
    }
}

#[test]
fn round_trip() {
    let tmp = TempDir::new().unwrap();
    let lockfile_path = tmp.path().join("plugins.lock");

    let original = sample_lockfile();
    original.write(&lockfile_path).unwrap();
    let loaded = Lockfile::load(&lockfile_path).unwrap();

    assert_eq!(loaded.schema_version, 1);
    assert_eq!(loaded.plugins.len(), 1);

    let plugin = &loaded.plugins["yara-scanner"];
    assert_eq!(plugin.version, "0.1.0");
    assert_eq!(plugin.repository, "malbox-plugins/yara-scanner");
    assert!(matches!(plugin.source, InstallSource::Registry));
    assert!(matches!(plugin.install_method, InstallMethod::Prebuilt));
    assert_eq!(
        plugin.asset.as_deref(),
        Some("yara-scanner-v0.1.0-x86_64-linux.tar.gz")
    );
}

#[test]
fn load_missing_file_returns_empty() {
    let tmp = TempDir::new().unwrap();
    let lockfile_path = tmp.path().join("plugins.lock");

    let lockfile = Lockfile::load(&lockfile_path).unwrap();
    assert!(lockfile.plugins.is_empty());
}

#[test]
fn detect_untracked_plugins() {
    let tmp = TempDir::new().unwrap();
    let plugins_dir = tmp.path().join("plugins");
    std::fs::create_dir(&plugins_dir).unwrap();

    // Create two plugin directories
    std::fs::create_dir(plugins_dir.join("yara-scanner")).unwrap();
    std::fs::create_dir(plugins_dir.join("custom-plugin")).unwrap();

    // Lockfile only tracks yara-scanner
    let lockfile = sample_lockfile();
    let untracked = lockfile.untracked_plugins(&plugins_dir).unwrap();

    assert_eq!(untracked.len(), 1);
    assert_eq!(untracked[0], "custom-plugin");
}

#[test]
fn insert_and_remove() {
    let mut lockfile = Lockfile::empty();

    lockfile.plugins.insert(
        "test-plugin".to_string(),
        LockedPlugin {
            version: "1.0.0".to_string(),
            repository: "user/test-plugin".to_string(),
            source: InstallSource::Direct,
            install_method: InstallMethod::Prebuilt,
            asset: Some("test-plugin-v1.0.0-x86_64-linux.tar.gz".to_string()),
            checksum: None,
            installed_at: "2026-06-07T14:00:00Z".to_string(),
            pin: PinKind::Release,
            commit: None,
            path: None,
        },
    );
    assert_eq!(lockfile.plugins.len(), 1);

    lockfile.plugins.remove("test-plugin");
    assert!(lockfile.plugins.is_empty());
}

#[test]
fn source_serialization() {
    let json = serde_json::to_string(&InstallSource::Registry).unwrap();
    assert_eq!(json, "\"registry\"");

    let json = serde_json::to_string(&InstallSource::Direct).unwrap();
    assert_eq!(json, "\"direct\"");
}

#[test]
fn v1_lockfile_without_pin_loads_with_defaults() {
    // A schema v1 lockfile has no `pin`/`commit` keys; they must default.
    let json = r#"{
        "schema_version": 1,
        "plugins": {
            "yara-scanner": {
                "version": "0.1.0",
                "repository": "malbox-plugins/yara-scanner",
                "source": "registry",
                "install_method": "prebuilt",
                "asset": null,
                "checksum": null,
                "installed_at": "2026-06-07T12:00:00Z"
            }
        }
    }"#;
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("plugins.lock");
    std::fs::write(&path, json).unwrap();

    let loaded = Lockfile::load(&path).unwrap();
    let plugin = &loaded.plugins["yara-scanner"];
    assert!(matches!(plugin.pin, PinKind::Release));
    assert!(plugin.commit.is_none());
}

#[test]
fn branch_pin_round_trips() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("plugins.lock");

    let mut plugins = HashMap::new();
    plugins.insert(
        "dev-plugin".to_string(),
        LockedPlugin {
            version: "main".to_string(),
            repository: "user/dev-plugin".to_string(),
            source: InstallSource::Direct,
            install_method: InstallMethod::Source,
            asset: None,
            checksum: None,
            installed_at: "2026-07-07T12:00:00Z".to_string(),
            pin: PinKind::Branch {
                name: "main".to_string(),
            },
            commit: Some("a1b2c3d4e5f6".to_string()),
            path: None,
        },
    );
    Lockfile {
        schema_version: 2,
        plugins,
    }
    .write(&path)
    .unwrap();

    let loaded = Lockfile::load(&path).unwrap();
    let plugin = &loaded.plugins["dev-plugin"];
    assert!(matches!(&plugin.pin, PinKind::Branch { name } if name == "main"));
    assert_eq!(plugin.commit.as_deref(), Some("a1b2c3d4e5f6"));
}

#[test]
fn empty_lockfile_is_schema_v2() {
    assert_eq!(Lockfile::empty().schema_version, 2);
}
