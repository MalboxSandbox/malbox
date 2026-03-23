use serde::{Deserialize, Serialize};

/// A single provisioning step in the pipeline.
///
/// Each step runs a provisioner. If `snapshot` is set, a provider-level
/// snapshot is created after the step completes successfully.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProvisionStep {
    /// Provisioner type name (must match a registered provisioner, e.g., "ansible").
    #[serde(rename = "type")]
    pub provisioner_type: String,

    /// Optional snapshot name. When set, a snapshot is taken after this step
    /// and recorded in the `machine_snapshots` table.
    #[serde(default)]
    pub snapshot: Option<String>,

    /// Provisioner-specific configuration (passed through to the provisioner).
    #[serde(default = "default_step_config")]
    pub config: toml::Value,

    /// Guest plugin names deployed in this step.
    /// Stored in the snapshot's `guest_plugins` JSONB column.
    #[serde(default)]
    pub guest_plugins: Vec<String>,
}

/// Provisioning configuration.
///
/// Defines an ordered list of provisioning steps to run after machine allocation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProvisioningConfig {
    /// Ordered provisioning steps.
    pub steps: Vec<ProvisionStep>,
}

fn default_step_config() -> toml::Value {
    toml::Value::Table(toml::map::Map::new())
}
