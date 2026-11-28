use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
/// Top-level `index.json` listing every plugin in the registry.
pub struct RegistryIndex {
    /// Index format version.
    pub schema_version: u32,
    /// Generation timestamp as published by the registry.
    pub generated_at: String,
    /// Summary entries for all registered plugins.
    pub plugins: Vec<IndexEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
/// Summary of one plugin as listed in the registry index.
pub struct IndexEntry {
    /// Unique plugin name.
    pub name: String,
    /// Short human-readable description.
    pub description: String,
    /// Plugin type label (`host` or `guest`).
    #[serde(rename = "type")]
    pub plugin_type: String,
    #[serde(default)]
    /// Free-form categories used for search.
    pub categories: Vec<String>,
    /// GitHub `owner/repo` hosting the plugin.
    pub repository: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
/// Full per-plugin metadata from `plugins/<name>.json` in the registry.
pub struct PluginMetadata {
    /// Metadata format version.
    pub schema_version: u32,
    /// Unique plugin name.
    pub name: String,
    /// Short human-readable description.
    pub description: String,
    /// GitHub `owner/repo` hosting the plugin.
    pub repository: String,
    #[serde(default)]
    /// Plugin authors as free-form strings.
    pub authors: Vec<String>,
    #[serde(default)]
    /// SPDX license identifier, if published.
    pub license: Option<String>,
    /// Plugin type label (`host` or `guest`).
    #[serde(rename = "type")]
    pub plugin_type: String,
    #[serde(default)]
    /// Free-form categories used for search.
    pub categories: Vec<String>,
    #[serde(default)]
    /// Search keywords.
    pub keywords: Vec<String>,
    #[serde(default)]
    /// Oldest malbox version the plugin supports.
    pub min_malbox_version: Option<String>,
    #[serde(default)]
    /// Project homepage URL.
    pub homepage: Option<String>,
    #[serde(default)]
    /// Other registry plugins this plugin needs installed.
    pub requires: Vec<Dependency>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
/// A registry plugin required by another plugin.
pub struct Dependency {
    /// Name of the required plugin.
    pub name: String,
    /// Version requirement for the required plugin.
    pub version: String,
}

impl RegistryIndex {
    /// Look up an index entry by exact plugin name.
    #[must_use]
    pub fn find(&self, name: &str) -> Option<&IndexEntry> {
        self.plugins.iter().find(|e| e.name == name)
    }

    /// Case-insensitive substring search over names, descriptions and categories.
    #[must_use]
    pub fn search(&self, query: &str) -> Vec<&IndexEntry> {
        let query_lower = query.to_lowercase();
        self.plugins
            .iter()
            .filter(|entry| {
                entry.name.to_lowercase().contains(&query_lower)
                    || entry.description.to_lowercase().contains(&query_lower)
                    || entry
                        .categories
                        .iter()
                        .any(|c| c.to_lowercase().contains(&query_lower))
            })
            .collect()
    }
}
