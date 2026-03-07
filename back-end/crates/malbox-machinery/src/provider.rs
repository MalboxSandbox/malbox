//! Providers register themselves at compile time using the `inventory` crate.
//! The registry provides a way to look up providers by name and access their
//! capabilities.

use crate::provider::capabilities::{Allocate, Clone, GuestAccess, Migrate, Snapshot};
use std::sync::Arc;

pub mod capabilities;
pub mod config;

// Re-export toml::Value so providers don't need to depend on toml directly
pub use toml::Value as TomlValue;

// TODO: Refactor `ProviderHandle`, the struct would eventually get huge with capability additions
// and hard to maintain, for the time being, we keep this as functional. Should be
// modified in the future.

/// Handle to a provider with its capabilities.
///
/// This struct holds trait objects for each capability that a provider implements.
/// The Allocate capability is mandatory, while others are optional.
///
/// This struct is created by the derive macro and should not be constructed manually.
pub struct ProviderHandle {
    /// Provider name (e.g., "libvirt", "vmware").
    name: String,
    /// Allocate capability (mandatory - all providers must implement this).
    allocate: Arc<dyn Allocate>,
    /// Snapshot capability (optional).
    snapshot: Option<Arc<dyn Snapshot>>,
    /// Clone capability (optional).
    clone: Option<Arc<dyn Clone>>,
    /// Migrate capability (optional).
    migrate: Option<Arc<dyn Migrate>>,
    /// GuestAccess capability (optional).
    guest_access: Option<Arc<dyn GuestAccess>>,
}

impl ProviderHandle {
    /// Create a new provider handle with required Allocate capability.
    ///
    /// This is called by the macro-generated code. Provider authors should not
    /// call this directly - use the `#[derive(RegisterProvider)]` macro instead.
    pub fn new(
        name: String,
        allocate: Arc<dyn Allocate>,
        snapshot: Option<Arc<dyn Snapshot>>,
        clone: Option<Arc<dyn Clone>>,
        migrate: Option<Arc<dyn Migrate>>,
        guest_access: Option<Arc<dyn GuestAccess>>,
    ) -> Self {
        Self {
            name,
            allocate,
            snapshot,
            clone,
            migrate,
            guest_access,
        }
    }

    /// Get the provider name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Get the Allocate capability.
    ///
    /// This is always available since all providers must implement Allocate.
    pub fn allocate(&self) -> Arc<dyn Allocate> {
        self.allocate.clone()
    }

    /// Get the Snapshot capability if available.
    pub fn snapshot(&self) -> Option<Arc<dyn Snapshot>> {
        self.snapshot.clone()
    }

    /// Get the Clone capability if available.
    pub fn clone_capability(&self) -> Option<Arc<dyn Clone>> {
        self.clone.clone()
    }

    /// Get the Migrate capability if available.
    pub fn migrate(&self) -> Option<Arc<dyn Migrate>> {
        self.migrate.clone()
    }

    /// Get the GuestAccess capability if available.
    pub fn guest_access(&self) -> Option<Arc<dyn GuestAccess>> {
        self.guest_access.clone()
    }

    /// Check if provider has a specific capability by name.
    pub fn has_capability(&self, capability: &str) -> bool {
        match capability {
            "allocate" => true,
            "snapshot" => self.snapshot.is_some(),
            "clone" => self.clone.is_some(),
            "migrate" => self.migrate.is_some(),
            "guest_access" => self.guest_access.is_some(),
            _ => false,
        }
    }

    /// Get all capability names this provider supports.
    pub fn list_capabilities(&self) -> Vec<&'static str> {
        let mut caps = vec!["allocate"]; // Always present
        if self.snapshot.is_some() {
            caps.push("snapshot");
        }
        if self.clone.is_some() {
            caps.push("clone");
        }
        if self.migrate.is_some() {
            caps.push("migrate");
        }
        if self.guest_access.is_some() {
            caps.push("guest_access");
        }
        caps
    }
}

/// Metadata about a registered provider.
///
/// This struct is submitted to the inventory by provider crates.
/// It contains the provider name and a function to create a ProviderHandle.
pub struct ProviderMetadata {
    /// Unique name for this provider (e.g., "libvirt", "vmware").
    pub name: &'static str,
    /// Function to create a provider handle with capabilities.
    /// Takes provider-specific configuration as TOML value and returns a ProviderHandle.
    pub create: fn(&TomlValue) -> Result<ProviderHandle, Box<dyn std::error::Error + Send + Sync>>,
}

// Collect all registered providers at compile time
inventory::collect!(ProviderMetadata);

/// Get provider metadata by name.
pub fn get_provider_metadata(name: &str) -> Option<&'static ProviderMetadata> {
    inventory::iter::<ProviderMetadata>().find(|p| p.name == name)
}

/// List all registered provider names.
pub fn list_providers() -> Vec<&'static str> {
    inventory::iter::<ProviderMetadata>()
        .map(|p| p.name)
        .collect()
}

/// Create a provider handle by name with configuration.
///
/// This looks up the provider in the registry and calls its create function
/// with the provider-specific configuration.
pub fn create_provider(
    name: &str,
    config: &TomlValue,
) -> Result<ProviderHandle, Box<dyn std::error::Error + Send + Sync>> {
    let metadata = get_provider_metadata(name).ok_or_else(|| {
        format!(
            "Provider '{}' not found. Available providers: {:?}",
            name,
            list_providers()
        )
    })?;

    (metadata.create)(config)
}
