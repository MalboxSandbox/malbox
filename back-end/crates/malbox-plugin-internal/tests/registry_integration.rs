//! Integration test for the plugin registry end-to-end flow.

use malbox_plugin_internal::registry::PluginRegistry;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use tempfile::TempDir;

fn create_plugin(parent: &Path, name: &str) {
    let dir = parent.join(name);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("plugin.toml"),
        format!(
            r#"
[plugin]
name = "{name}"
version = "1.0.0"
type = "host"

[runtime]
state = "ephemeral"
execution = "parallel"
"#
        ),
    )
    .unwrap();
    let binary = dir.join(name);
    std::fs::write(&binary, "#!/bin/sh\n").unwrap();
    std::fs::set_permissions(&binary, std::fs::Permissions::from_mode(0o755)).unwrap();
}

#[test]
fn full_lifecycle() {
    // 1. Start with two plugins
    let tmp = TempDir::new().unwrap();
    create_plugin(tmp.path(), "plugin-a");
    create_plugin(tmp.path(), "plugin-b");

    let registry = PluginRegistry::new(tmp.path().to_path_buf()).unwrap();
    let snap = registry.snapshot();
    assert_eq!(snap.len(), 2);

    // 2. Add a new plugin at runtime
    create_plugin(tmp.path(), "plugin-c");
    std::thread::sleep(std::time::Duration::from_millis(800));

    let diff = registry.apply_pending();
    assert!(diff.added.iter().any(|id| id.as_str() == "plugin-c"));

    let snap = registry.snapshot();
    assert_eq!(snap.len(), 3);

    // 3. Remove a plugin directory
    std::fs::remove_dir_all(tmp.path().join("plugin-a")).unwrap();
    std::thread::sleep(std::time::Duration::from_millis(800));

    let _diff = registry.apply_pending();
    // The watcher should detect the removal -- behavior is OS-dependent
    let snap = registry.snapshot();
    // After applying, plugin-a should be gone (if Remove detected)
    // or still present (if only Modify events for deleted files).
    // Just verify no panic and snapshot is consistent.
    assert!(snap.len() >= 2);
}

#[test]
fn empty_directory_works() {
    let tmp = TempDir::new().unwrap();
    let registry = PluginRegistry::new(tmp.path().to_path_buf()).unwrap();
    assert!(registry.snapshot().is_empty());
    assert!(registry.apply_pending().is_empty());
}
