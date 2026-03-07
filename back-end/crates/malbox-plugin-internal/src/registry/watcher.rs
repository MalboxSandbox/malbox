use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher as NotifyWatcher};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use super::error::WatcherError;
use super::types::PluginId;

/// A pending change detected by the filesystem watcher.
#[derive(Debug, Clone)]
pub enum PendingChange {
    Added(PathBuf),
    Modified(PathBuf),
    Removed(PluginId),
}

/// Filesystem watcher that monitors the plugin directory for changes.
pub struct Watcher {
    _watcher: RecommendedWatcher,
    pending: Arc<Mutex<Vec<PendingChange>>>,
}

impl Watcher {
    pub fn start(plugin_dir: &Path) -> Result<Self, WatcherError> {
        let pending: Arc<Mutex<Vec<PendingChange>>> = Arc::new(Mutex::new(Vec::new()));
        let pending_clone = Arc::clone(&pending);
        let plugin_dir_owned = plugin_dir.to_path_buf();

        let debounce_state: Arc<Mutex<std::collections::HashMap<PathBuf, Instant>>> =
            Arc::new(Mutex::new(std::collections::HashMap::new()));
        let debounce_window = Duration::from_millis(500);

        let mut watcher =
            notify::recommended_watcher(move |res: Result<Event, notify::Error>| {
                let event = match res {
                    Ok(e) => e,
                    Err(e) => {
                        tracing::error!(error = %e, "Filesystem watcher error");
                        return;
                    }
                };

                for path in &event.paths {
                    let plugin_subdir = match resolve_plugin_dir(path, &plugin_dir_owned) {
                        Some(d) => d,
                        None => continue,
                    };

                    {
                        let mut state = debounce_state.lock().unwrap();
                        let now = Instant::now();
                        if let Some(last) = state.get(&plugin_subdir) {
                            if now.duration_since(*last) < debounce_window {
                                continue;
                            }
                        }
                        state.insert(plugin_subdir.clone(), now);
                    }

                    let change = match event.kind {
                        EventKind::Create(_) => PendingChange::Added(plugin_subdir),
                        EventKind::Modify(_) => PendingChange::Modified(plugin_subdir),
                        EventKind::Remove(_) => {
                            let name = path
                                .file_name()
                                .map(|n| n.to_string_lossy().into_owned())
                                .unwrap_or_default();
                            PendingChange::Removed(PluginId::new(name))
                        }
                        _ => continue,
                    };

                    let mut pending = pending_clone.lock().unwrap();
                    pending.push(change);
                }
            })?;

        watcher.watch(plugin_dir, RecursiveMode::Recursive)?;

        Ok(Self {
            _watcher: watcher,
            pending,
        })
    }

    pub fn drain_pending(&self) -> Vec<PendingChange> {
        let mut pending = self.pending.lock().unwrap();
        std::mem::take(&mut *pending)
    }
}

fn resolve_plugin_dir(event_path: &Path, plugin_dir: &Path) -> Option<PathBuf> {
    let relative = event_path.strip_prefix(plugin_dir).ok()?;
    let first_component = relative.components().next()?;
    let subdir = plugin_dir.join(first_component);
    Some(subdir)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use std::time::Duration;
    use tempfile::TempDir;

    #[test]
    fn drain_pending_empty_initially() {
        let tmp = TempDir::new().unwrap();
        let watcher = Watcher::start(tmp.path()).unwrap();
        let changes = watcher.drain_pending();
        assert!(changes.is_empty());
    }

    #[test]
    fn detects_new_directory() {
        let tmp = TempDir::new().unwrap();
        let watcher = Watcher::start(tmp.path()).unwrap();

        let plugin_dir = tmp.path().join("new-plugin");
        std::fs::create_dir_all(&plugin_dir).unwrap();
        std::fs::write(plugin_dir.join("plugin.toml"), "test").unwrap();

        thread::sleep(Duration::from_millis(800));

        let changes = watcher.drain_pending();
        assert!(!changes.is_empty());
        let has_relevant = changes
            .iter()
            .any(|c| matches!(c, PendingChange::Added(_) | PendingChange::Modified(_)));
        assert!(has_relevant);
    }

    #[test]
    fn drain_clears_pending() {
        let tmp = TempDir::new().unwrap();
        let watcher = Watcher::start(tmp.path()).unwrap();

        std::fs::create_dir_all(tmp.path().join("plugin-x")).unwrap();
        thread::sleep(Duration::from_millis(800));

        let first = watcher.drain_pending();
        assert!(!first.is_empty());

        let second = watcher.drain_pending();
        assert!(second.is_empty());
    }
}
