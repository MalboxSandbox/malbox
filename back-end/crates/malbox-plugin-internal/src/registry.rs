pub mod error;
pub mod manifest;
pub mod scanner;
pub mod snapshot;
pub mod types;
pub mod watcher;

use arc_swap::ArcSwap;
use std::path::PathBuf;
use std::sync::Arc;

use error::RegistryError;
use scanner::Scanner;
use snapshot::{PluginSnapshot, PluginSnapshotInner};
use types::PluginId;
use watcher::{PendingChange, Watcher};

/// Tracks what changed during an `apply_pending()` call.
#[derive(Debug, Default)]
pub struct RegistryDiff {
    pub added: Vec<PluginId>,
    pub updated: Vec<PluginId>,
    pub removed: Vec<PluginId>,
    pub errors: Vec<(PathBuf, String)>,
}

impl RegistryDiff {
    pub fn is_empty(&self) -> bool {
        self.added.is_empty()
            && self.updated.is_empty()
            && self.removed.is_empty()
            && self.errors.is_empty()
    }
}

/// The plugin registry: discovers, tracks, and provides access to plugins.
pub struct PluginRegistry {
    scanner: Scanner,
    watcher: Watcher,
    current: ArcSwap<PluginSnapshotInner>,
}

impl PluginRegistry {
    /// Create the registry, scan for existing plugins, and start watching for changes.
    pub fn new(plugin_dir: PathBuf) -> Result<Self, RegistryError> {
        let scanner = Scanner::new(plugin_dir.clone());
        let entries = scanner.scan_all()?;

        tracing::info!(
            count = entries.len(),
            dir = %plugin_dir.display(),
            "Initial plugin scan complete"
        );

        let plugins = entries
            .into_iter()
            .map(|e| (e.id.clone(), Arc::new(e)))
            .collect();
        let snapshot = PluginSnapshotInner::new(plugins);

        let watcher = Watcher::start(&plugin_dir)?;

        Ok(Self {
            scanner,
            watcher,
            current: ArcSwap::from_pointee(snapshot),
        })
    }

    /// Get the current plugin snapshot. Cheap (Arc clone).
    pub fn snapshot(&self) -> PluginSnapshot {
        PluginSnapshot::from_inner(self.current.load_full())
    }

    /// Apply any pending filesystem changes. Returns a diff of what changed.
    pub fn apply_pending(&self) -> RegistryDiff {
        let changes = self.watcher.drain_pending();
        if changes.is_empty() {
            return RegistryDiff::default();
        }

        let mut plugins = self.current.load().plugins.clone();
        let mut diff = RegistryDiff::default();

        for change in changes {
            match change {
                PendingChange::Added(path) | PendingChange::Modified(path) => {
                    match self.scanner.scan_one(&path) {
                        Ok(entry) => {
                            let id = entry.id.clone();
                            if plugins.contains_key(&id) {
                                diff.updated.push(id.clone());
                            } else {
                                diff.added.push(id.clone());
                            }
                            plugins.insert(id, Arc::new(entry));
                        }
                        Err(e) => {
                            diff.errors.push((path, e.to_string()));
                        }
                    }
                }
                PendingChange::Removed(id) => {
                    if plugins.remove(&id).is_some() {
                        diff.removed.push(id);
                    }
                }
            }
        }

        self.current
            .store(Arc::new(PluginSnapshotInner::new(plugins)));
        diff
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;
    use std::path::Path;
    use std::thread;
    use std::time::Duration;
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
    fn new_discovers_existing_plugins() {
        let tmp = TempDir::new().unwrap();
        create_plugin(tmp.path(), "plugin-a");
        create_plugin(tmp.path(), "plugin-b");

        let registry = PluginRegistry::new(tmp.path().to_path_buf()).unwrap();
        let snapshot = registry.snapshot();

        assert_eq!(snapshot.len(), 2);
    }

    #[test]
    fn snapshot_is_consistent() {
        let tmp = TempDir::new().unwrap();
        create_plugin(tmp.path(), "my-plugin");

        let registry = PluginRegistry::new(tmp.path().to_path_buf()).unwrap();
        let snap1 = registry.snapshot();
        let snap2 = registry.snapshot();

        assert_eq!(snap1.len(), snap2.len());
        assert_eq!(snap1.len(), 1);
    }

    #[test]
    fn apply_pending_no_changes() {
        let tmp = TempDir::new().unwrap();
        let registry = PluginRegistry::new(tmp.path().to_path_buf()).unwrap();
        let diff = registry.apply_pending();
        assert!(diff.is_empty());
    }

    #[test]
    fn apply_pending_picks_up_new_plugin() {
        let tmp = TempDir::new().unwrap();
        let registry = PluginRegistry::new(tmp.path().to_path_buf()).unwrap();

        assert_eq!(registry.snapshot().len(), 0);

        create_plugin(tmp.path(), "new-plugin");
        thread::sleep(Duration::from_millis(800));

        let diff = registry.apply_pending();
        assert!(!diff.is_empty());
        assert_eq!(registry.snapshot().len(), 1);
    }

    #[test]
    fn new_with_nonexistent_dir_fails() {
        let result = PluginRegistry::new(PathBuf::from("/nonexistent/plugins"));
        assert!(result.is_err());
    }

    #[test]
    fn empty_dir_creates_empty_registry() {
        let tmp = TempDir::new().unwrap();
        let registry = PluginRegistry::new(tmp.path().to_path_buf()).unwrap();
        assert!(registry.snapshot().is_empty());
    }
}
