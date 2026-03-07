//! Provider configuration management.
//!
//! This module handles configuration for virtualization providers (libvirt, VMware, VirtualBox, etc.).
//! Each provider can have its own configuration schema, which is stored as generic TOML values
//! and deserialized on-demand when the provider is initialized.

use crate::ConfigError;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Configuration for all providers.
///
/// This structure manages which providers are enabled and stores provider-specific
/// configuration as generic TOML values that can be deserialized into provider-specific
/// config structs when needed.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProvidersConfig {
    /// List of enabled providers.
    ///
    /// These providers must be compiled into the daemon via Cargo features.
    /// Example: `["libvirt", "vmware"]`
    #[serde(default)]
    pub enabled: Vec<String>,

    /// Default provider to use when not explicitly specified.
    ///
    /// Must be one of the enabled providers.
    pub default: Option<String>,

    /// Provider-specific configurations stored as generic TOML values.
    ///
    /// Each provider defines its own config schema and deserializes it when needed.
    /// This allows extensibility without hardcoding provider types.
    ///
    /// Example in TOML:
    /// ```toml
    /// [providers.libvirt]
    /// uri = "qemu:///system"
    /// storage_pool = "default"
    /// ```
    #[serde(flatten)]
    pub configs: HashMap<String, toml::Value>,
}

impl ProvidersConfig {
    /// Get configuration for a specific provider.
    ///
    /// Deserializes the provider's configuration into the requested type.
    pub fn get_provider_config<T>(&self, name: &str) -> Result<T, ConfigError>
    where
        T: serde::de::DeserializeOwned,
    {
        let value = self
            .configs
            .get(name)
            .ok_or_else(|| ConfigError::ProviderNotConfigured(name.to_string()))?;

        // Convert the TOML value to a string and deserialize it
        let toml_str = toml::to_string(value).map_err(|e| ConfigError::Parse {
            file: format!("providers.{}", name),
            error: e.to_string(),
        })?;

        toml::from_str(&toml_str).map_err(|e| ConfigError::InvalidProviderConfig {
            provider: name.to_string(),
            error: e.to_string(),
        })
    }

    /// Check if a provider is enabled.
    pub fn is_enabled(&self, name: &str) -> bool {
        self.enabled.iter().any(|p| p == name)
    }

    /// Get the default provider name.
    pub fn get_default(&self) -> Option<&str> {
        self.default.as_deref()
    }

    /// Add a provider to the enabled list.
    ///
    /// Returns `true` if the provider was added, `false` if it was already enabled.
    pub fn enable(&mut self, name: String) -> bool {
        if !self.is_enabled(&name) {
            self.enabled.push(name);
            true
        } else {
            false
        }
    }

    /// Remove a provider from the enabled list.
    ///
    /// Returns `true` if the provider was removed, `false` if it wasn't enabled.
    pub fn disable(&mut self, name: &str) -> bool {
        if let Some(pos) = self.enabled.iter().position(|p| p == name) {
            self.enabled.remove(pos);
            // Clear default if we're disabling the default provider
            if self.default.as_deref() == Some(name) {
                self.default = None;
            }
            true
        } else {
            false
        }
    }

    /// Set the default provider.
    pub fn set_default(&mut self, name: String) -> Result<(), ConfigError> {
        if !self.is_enabled(&name) {
            return Err(ConfigError::ProviderNotEnabled(name));
        }
        self.default = Some(name);
        Ok(())
    }

    /// Validate that enabled providers match what's compiled into the daemon.
    pub fn validate(&self, compiled: &[String]) -> Result<(), ConfigError> {
        let mut config_sorted = self.enabled.clone();
        config_sorted.sort();
        let mut compiled_sorted = compiled.to_vec();
        compiled_sorted.sort();

        if config_sorted != compiled_sorted {
            let mut message = String::from("Provider configuration mismatch\n\n");

            message.push_str("Providers compiled into daemon:\n");
            for provider in &compiled_sorted {
                message.push_str(&format!("  - {}\n", provider));
            }

            message.push_str("\nProviders in config:\n");
            if self.enabled.is_empty() {
                message.push_str("  (none)\n");
            } else {
                for provider in &config_sorted {
                    message.push_str(&format!("  - {}\n", provider));
                }
            }

            message.push_str(
                "\nTo fix this, add to your config file (~/.config/malbox/malbox.toml):\n\n",
            );
            message.push_str("[providers]\n");
            message.push_str(&format!("enabled = {:?}\n", compiled_sorted));

            return Err(ConfigError::Internal(message));
        }

        Ok(())
    }
}
