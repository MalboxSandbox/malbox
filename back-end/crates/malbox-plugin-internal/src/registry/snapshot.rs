use std::collections::HashMap;
use std::sync::Arc;
use std::time::SystemTime;

use super::manifest::{ExecutionContextConfig, PluginTypeConfig};
use super::types::{PluginEntry, PluginId};

/// Immutable, cheaply cloneable snapshot of all registered plugins.
#[derive(Clone)]
pub struct PluginSnapshot {
    inner: Arc<PluginSnapshotInner>,
}

pub(crate) struct PluginSnapshotInner {
    pub(crate) plugins: HashMap<PluginId, Arc<PluginEntry>>,
    pub(crate) created_at: SystemTime,
}

impl PluginSnapshot {
    pub fn from_entries(entries: Vec<PluginEntry>) -> Self {
        let plugins = entries
            .into_iter()
            .map(|e| (e.id.clone(), Arc::new(e)))
            .collect();

        Self {
            inner: Arc::new(PluginSnapshotInner {
                plugins,
                created_at: SystemTime::now(),
            }),
        }
    }

    pub(crate) fn from_inner(inner: Arc<PluginSnapshotInner>) -> Self {
        Self { inner }
    }

    pub fn get(&self, id: &PluginId) -> Option<&Arc<PluginEntry>> {
        self.inner.plugins.get(id)
    }

    pub fn list(&self) -> impl Iterator<Item = &Arc<PluginEntry>> {
        self.inner.plugins.values()
    }

    pub fn by_type(&self, t: PluginTypeConfig) -> Vec<&Arc<PluginEntry>> {
        self.inner
            .plugins
            .values()
            .filter(|e| e.manifest.plugin.plugin_type == t)
            .collect()
    }

    pub fn by_execution_context(&self, ctx: ExecutionContextConfig) -> Vec<&Arc<PluginEntry>> {
        self.inner
            .plugins
            .values()
            .filter(|e| e.manifest.runtime.execution == ctx)
            .collect()
    }

    pub fn len(&self) -> usize {
        self.inner.plugins.len()
    }

    pub fn is_empty(&self) -> bool {
        self.inner.plugins.is_empty()
    }

    pub fn created_at(&self) -> SystemTime {
        self.inner.created_at
    }
}

impl PluginSnapshotInner {
    pub fn new(plugins: HashMap<PluginId, Arc<PluginEntry>>) -> Self {
        Self {
            plugins,
            created_at: SystemTime::now(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::manifest::*;
    use std::collections::HashMap;
    use std::path::PathBuf;
    use std::time::SystemTime;

    fn make_entry(
        name: &str,
        plugin_type: PluginTypeConfig,
        execution: ExecutionContextConfig,
    ) -> PluginEntry {
        use malbox_plugin_manifest::{PathsConfig, StashConfig};
        PluginEntry {
            id: PluginId::new(name),
            manifest: PluginManifest {
                plugin: PluginInfo {
                    name: name.to_string(),
                    version: "1.0.0".to_string(),
                    description: None,
                    authors: vec![],
                    plugin_type,
                    binary: None,
                },
                scope: None,
                results: HashMap::new(),
                events: None,
                runtime: RuntimeConfig {
                    state: PluginStateConfig::Ephemeral,
                    execution,
                    port: None,
                    log_filter: None,
                    paths: PathsConfig::default(),
                    stash: StashConfig::default(),
                    auto_collect: Default::default(),
                },
            },
            binary_path: PathBuf::from(format!("/plugins/{name}/{name}")),
            plugin_dir: PathBuf::from(format!("/plugins/{name}")),
            registered_at: SystemTime::now(),
            status: crate::registry::types::PluginStatus::Registered,
            runtime_config: None,
        }
    }

    #[test]
    fn empty_snapshot() {
        let snapshot = PluginSnapshot::from_entries(vec![]);
        assert_eq!(snapshot.len(), 0);
        assert!(snapshot.is_empty());
    }

    #[test]
    fn get_by_id() {
        let entries = vec![make_entry(
            "pe-parser",
            PluginTypeConfig::Host,
            ExecutionContextConfig::Parallel,
        )];
        let snapshot = PluginSnapshot::from_entries(entries);

        let id = PluginId::new("pe-parser");
        assert!(snapshot.get(&id).is_some());
        assert_eq!(snapshot.get(&id).unwrap().id.as_str(), "pe-parser");

        let missing = PluginId::new("nonexistent");
        assert!(snapshot.get(&missing).is_none());
    }

    #[test]
    fn list_all() {
        let entries = vec![
            make_entry(
                "a",
                PluginTypeConfig::Host,
                ExecutionContextConfig::Parallel,
            ),
            make_entry(
                "b",
                PluginTypeConfig::Guest,
                ExecutionContextConfig::Exclusive,
            ),
        ];
        let snapshot = PluginSnapshot::from_entries(entries);
        assert_eq!(snapshot.list().count(), 2);
    }

    #[test]
    fn filter_by_type() {
        let entries = vec![
            make_entry(
                "host-a",
                PluginTypeConfig::Host,
                ExecutionContextConfig::Parallel,
            ),
            make_entry(
                "host-b",
                PluginTypeConfig::Host,
                ExecutionContextConfig::Sequential,
            ),
            make_entry(
                "guest-a",
                PluginTypeConfig::Guest,
                ExecutionContextConfig::Parallel,
            ),
        ];
        let snapshot = PluginSnapshot::from_entries(entries);

        assert_eq!(snapshot.by_type(PluginTypeConfig::Host).len(), 2);
        assert_eq!(snapshot.by_type(PluginTypeConfig::Guest).len(), 1);
    }

    #[test]
    fn filter_by_execution_context() {
        let entries = vec![
            make_entry(
                "a",
                PluginTypeConfig::Host,
                ExecutionContextConfig::Parallel,
            ),
            make_entry(
                "b",
                PluginTypeConfig::Host,
                ExecutionContextConfig::Parallel,
            ),
            make_entry(
                "c",
                PluginTypeConfig::Host,
                ExecutionContextConfig::Exclusive,
            ),
        ];
        let snapshot = PluginSnapshot::from_entries(entries);

        assert_eq!(
            snapshot
                .by_execution_context(ExecutionContextConfig::Parallel)
                .len(),
            2
        );
        assert_eq!(
            snapshot
                .by_execution_context(ExecutionContextConfig::Exclusive)
                .len(),
            1
        );
        assert_eq!(
            snapshot
                .by_execution_context(ExecutionContextConfig::Sequential)
                .len(),
            0
        );
    }

    #[test]
    fn snapshot_is_clone_cheap() {
        let entries = vec![make_entry(
            "a",
            PluginTypeConfig::Host,
            ExecutionContextConfig::Parallel,
        )];
        let snapshot = PluginSnapshot::from_entries(entries);
        let cloned = snapshot.clone();
        assert_eq!(snapshot.len(), cloned.len());
    }
}
