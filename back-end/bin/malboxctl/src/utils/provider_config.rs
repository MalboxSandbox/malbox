//! Comment-preserving edits of the daemon config's `[providers]` section.
//!
//! Uses `toml_edit` rather than round-tripping through the typed `Config`:
//! a user's hand-edited `malbox.toml` keeps its comments and formatting.

use malbox_cli_common::error::{CliError, Result};
use std::path::{Path, PathBuf};
use toml_edit::DocumentMut;

pub struct ProvidersEdit {
    doc: DocumentMut,
    path: PathBuf,
}

impl ProvidersEdit {
    /// Load the config file the daemon would use, mirroring its discovery
    /// order (`MALBOX_CONFIG` override, then user, then system config).
    pub fn load() -> Result<Self> {
        let path = malbox_config::config_file_path().ok_or_else(|| {
            CliError::CommandFailed(
                "no configuration found - run `malboxctl install` (or `malboxctl config init`) first"
                    .to_string(),
            )
        })?;
        let content = std::fs::read_to_string(&path).map_err(|e| {
            CliError::CommandFailed(format!("failed to read {}: {e}", path.display()))
        })?;
        let doc = content.parse::<DocumentMut>().map_err(|e| {
            CliError::CommandFailed(format!("failed to parse {}: {e}", path.display()))
        })?;
        Ok(Self { doc, path })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn enabled(&self) -> Vec<String> {
        self.doc
            .get("providers")
            .and_then(|p| p.get("enabled"))
            .and_then(|e| e.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default()
    }

    pub fn default_provider(&self) -> Option<String> {
        self.doc
            .get("providers")
            .and_then(|p| p.get("default"))
            .and_then(|d| d.as_str())
            .map(str::to_string)
    }

    /// Enable a provider. Returns `false` when it was already enabled. The
    /// first enabled provider becomes the default.
    pub fn add(&mut self, name: &str) -> bool {
        let mut enabled = self.enabled();
        if enabled.iter().any(|p| p == name) {
            return false;
        }
        enabled.push(name.to_string());
        self.write_enabled(&enabled);
        if self.default_provider().is_none() {
            self.doc["providers"]["default"] = toml_edit::value(name);
        }
        true
    }

    /// Disable a provider. Returns `false` when it was not enabled. A
    /// default pointing at the removed provider moves to the first
    /// remaining one (or is dropped).
    pub fn remove(&mut self, name: &str) -> bool {
        let mut enabled = self.enabled();
        let before = enabled.len();
        enabled.retain(|p| p != name);
        if enabled.len() == before {
            return false;
        }
        self.write_enabled(&enabled);
        if self.default_provider().as_deref() == Some(name) {
            self.reset_default(&enabled);
        }
        true
    }

    /// Replace the enabled set wholesale (used by `upgrade --reconfigure`),
    /// keeping the default valid.
    pub fn set_enabled(&mut self, providers: &[String]) {
        self.write_enabled(providers);
        let default_ok = self
            .default_provider()
            .is_some_and(|d| providers.contains(&d));
        if !default_ok {
            self.reset_default(providers);
        }
    }

    pub fn save(&self) -> Result<()> {
        std::fs::write(&self.path, self.doc.to_string()).map_err(|e| {
            CliError::CommandFailed(format!("failed to write {}: {e}", self.path.display()))
        })
    }

    fn write_enabled(&mut self, providers: &[String]) {
        let mut arr = toml_edit::Array::new();
        for provider in providers {
            arr.push(provider.as_str());
        }
        self.doc["providers"]["enabled"] = toml_edit::value(arr);
    }

    fn reset_default(&mut self, enabled: &[String]) {
        match enabled.first() {
            Some(first) => self.doc["providers"]["default"] = toml_edit::value(first.as_str()),
            None => {
                if let Some(table) = self.doc["providers"].as_table_like_mut() {
                    table.remove("default");
                }
            }
        }
    }
}
