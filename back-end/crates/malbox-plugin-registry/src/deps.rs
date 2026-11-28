use crate::client::Cache;
use crate::index::PluginMetadata;
use crate::lockfile::{InstallSource, Lockfile};
use std::path::Path;

/// A registry dependency declared by an installed plugin but absent from the lockfile.
#[derive(Debug)]
pub struct MissingDep {
    /// Installed plugin that declares the dependency.
    pub plugin: String,
    /// Name of the missing dependency plugin.
    pub dep_name: String,
    /// Version requirement declared for the dependency.
    pub dep_version: String,
}

/// List registry dependencies of installed plugins that are not installed, using
/// only cached metadata; unreadable lockfiles or cache entries are skipped silently.
#[must_use]
pub fn check_startup_deps(plugins_dir: &Path, cache_dir: &Path) -> Vec<MissingDep> {
    let mut missing = Vec::new();

    let lockfile_path = Lockfile::lockfile_path(plugins_dir);
    let Ok(lockfile) = Lockfile::load(&lockfile_path) else {
        return missing;
    };

    let cache = Cache::new(cache_dir.to_path_buf());

    for (name, plugin) in &lockfile.plugins {
        if !matches!(plugin.source, InstallSource::Registry) {
            continue;
        }

        let key = format!("plugins/{name}.json");
        let Ok(Some((bytes, _))) = cache.read(&key) else {
            continue;
        };

        let Ok(metadata) = serde_json::from_slice::<PluginMetadata>(&bytes) else {
            continue;
        };

        for dep in &metadata.requires {
            if !lockfile.plugins.contains_key(&dep.name) {
                missing.push(MissingDep {
                    plugin: name.clone(),
                    dep_name: dep.name.clone(),
                    dep_version: dep.version.clone(),
                });
            }
        }
    }

    missing
}
