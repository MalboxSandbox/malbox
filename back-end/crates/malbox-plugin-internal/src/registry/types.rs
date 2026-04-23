use std::fmt;
use std::path::PathBuf;
use std::time::SystemTime;

use malbox_plugin_manifest::ResolvedRuntimeConfig;

/// Unique identifier for a plugin, derived from its manifest name.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PluginId(String);

impl PluginId {
    pub fn new(name: impl Into<String>) -> Self {
        Self(name.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for PluginId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Registration status of a discovered plugin.
#[derive(Debug, Clone)]
pub enum PluginStatus {
    /// Plugin is valid and available for use.
    Registered,
    /// Plugin was discovered but has issues.
    Invalid(String),
}

impl fmt::Display for PluginStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Registered => write!(f, "registered"),
            Self::Invalid(reason) => write!(f, "invalid: {}", reason),
        }
    }
}

/// A discovered plugin with its metadata and filesystem location.
#[derive(Debug, Clone)]
pub struct PluginEntry {
    pub id: PluginId,
    pub manifest: super::manifest::PluginManifest,
    pub binary_path: PathBuf,
    pub plugin_dir: PathBuf,
    pub registered_at: SystemTime,
    pub status: PluginStatus,
    /// Resolved runtime settings (defaults filled in). `None` if validation couldn't be attempted.
    pub runtime_config: Option<ResolvedRuntimeConfig>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plugin_id_from_string() {
        let id = PluginId::new("pe-parser");
        assert_eq!(id.as_str(), "pe-parser");
    }

    #[test]
    fn plugin_id_equality() {
        let a = PluginId::new("yara-scanner");
        let b = PluginId::new("yara-scanner");
        assert_eq!(a, b);
    }

    #[test]
    fn plugin_status_display() {
        assert!(format!("{}", PluginStatus::Registered).contains("registered"));
        let invalid = PluginStatus::Invalid("missing binary".into());
        assert!(format!("{}", invalid).contains("missing binary"));
    }
}
