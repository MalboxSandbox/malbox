use serde::{Deserialize, Serialize};

/// Provisioning configuration.
///
/// Defines which provisioner to use and its settings.
/// When present, machines are provisioned after allocation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProvisioningConfig {
    /// Provisioner type name (must match a registered provisioner, e.g., "ansible").
    #[serde(rename = "type")]
    pub provisioner_type: String,
    /// Provisioner-specific configuration (passed through to the provisioner).
    #[serde(default = "default_provisioner_config")]
    pub config: toml::Value,
}

fn default_provisioner_config() -> toml::Value {
    toml::Value::Table(toml::map::Map::new())
}
