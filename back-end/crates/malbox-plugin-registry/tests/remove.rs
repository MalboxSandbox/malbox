use malbox_plugin_registry::lockfile::{
    InstallMethod, InstallSource, LockedPlugin, Lockfile, PinKind,
};
use malbox_plugin_registry::remove::remove_plugin;
use std::collections::HashMap;
use tempfile::TempDir;

#[test]
fn remove_tracked_plugin() {
    let tmp = TempDir::new().unwrap();
    let plugins_dir = tmp.path().join("plugins");
    let plugin_dir = plugins_dir.join("test-plugin");
    std::fs::create_dir_all(&plugin_dir).unwrap();
    std::fs::write(plugin_dir.join("plugin.toml"), "dummy").unwrap();

    let lockfile_path = Lockfile::lockfile_path(&plugins_dir);
    let mut plugins = HashMap::new();
    plugins.insert(
        "test-plugin".to_string(),
        LockedPlugin {
            version: "0.1.0".into(),
            repository: "user/test-plugin".into(),
            source: InstallSource::Registry,
            install_method: InstallMethod::Prebuilt,
            asset: None,
            checksum: None,
            installed_at: "2026-06-07T12:00:00Z".into(),
            pin: PinKind::Release,
            commit: None,
            path: None,
        },
    );
    let lockfile = Lockfile {
        schema_version: 1,
        plugins,
    };
    lockfile.write(&lockfile_path).unwrap();

    remove_plugin("test-plugin", &plugins_dir).unwrap();

    assert!(!plugin_dir.exists());
    let updated = Lockfile::load(&lockfile_path).unwrap();
    assert!(!updated.plugins.contains_key("test-plugin"));
}

#[test]
fn remove_untracked_plugin() {
    let tmp = TempDir::new().unwrap();
    let plugins_dir = tmp.path().join("plugins");
    let plugin_dir = plugins_dir.join("untracked");
    std::fs::create_dir_all(&plugin_dir).unwrap();
    std::fs::write(plugin_dir.join("plugin.toml"), "dummy").unwrap();

    let lockfile_path = Lockfile::lockfile_path(&plugins_dir);
    Lockfile::empty().write(&lockfile_path).unwrap();

    remove_plugin("untracked", &plugins_dir).unwrap();
    assert!(!plugin_dir.exists());
}

#[test]
fn remove_nonexistent_plugin() {
    let tmp = TempDir::new().unwrap();
    let plugins_dir = tmp.path().join("plugins");
    std::fs::create_dir_all(&plugins_dir).unwrap();

    let result = remove_plugin("nonexistent", &plugins_dir);
    assert!(result.is_err());
}
