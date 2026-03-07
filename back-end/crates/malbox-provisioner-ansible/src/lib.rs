//! Ansible provisioner for malbox.
//!
//! This crate provides a [`Provisioner`] implementation that runs Ansible
//! playbooks against allocated machines. It generates a dynamic inventory
//! when none is specified and forwards provisioner-specific configuration
//! as extra variables.

pub mod error;

use std::collections::HashMap;
use std::path::PathBuf;

use async_trait::async_trait;
use serde::Deserialize;
use tracing::{debug, info, warn};

use malbox_machinery::provisioner::config::deserialize;
use malbox_machinery::provisioner::{
    ProvisionContext, ProvisionResult, ProvisionStatus, Provisioner, ProvisionerMetadata,
};

use crate::error::AnsibleError;

/// Configuration for the Ansible provisioner.
///
/// Deserialized from the TOML provisioner config block.
#[derive(Debug, Deserialize)]
struct AnsibleConfig {
    /// Path to the Ansible playbook to execute (required).
    playbook: PathBuf,

    /// Inventory source. Set to `"dynamic"` (the default) to auto-generate
    /// a temporary inventory containing the machine IP.
    #[serde(default = "default_inventory")]
    inventory: String,

    /// Extra variables passed to `ansible-playbook --extra-vars`.
    #[serde(default)]
    extra_vars: HashMap<String, String>,

    /// Timeout in seconds for the ansible-playbook process.
    #[serde(default = "default_timeout")]
    timeout: u64,

    /// Path to the `ansible-playbook` binary.
    #[serde(default = "default_ansible_bin")]
    ansible_bin: String,
}

fn default_inventory() -> String {
    "dynamic".to_string()
}

fn default_timeout() -> u64 {
    600
}

fn default_ansible_bin() -> String {
    "ansible-playbook".to_string()
}

/// Ansible provisioner.
///
/// Runs an Ansible playbook against the target machine using
/// `ansible-playbook`. When the inventory is set to `"dynamic"`, a temporary
/// inventory file is generated containing only the machine's IP address.
pub struct AnsibleProvisioner {
    config: AnsibleConfig,
}

impl AnsibleProvisioner {
    /// Create a new Ansible provisioner from a TOML configuration value.
    ///
    /// Validates that the configured playbook path exists.
    pub fn new(
        config: &malbox_machinery::provisioner::TomlValue,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let config: AnsibleConfig = deserialize(config)?;

        if !config.playbook.exists() {
            return Err(Box::new(AnsibleError::Config(format!(
                "Playbook path does not exist: {}",
                config.playbook.display()
            ))));
        }

        Ok(Self { config })
    }
}

#[async_trait]
impl Provisioner for AnsibleProvisioner {
    async fn provision(
        &self,
        context: &ProvisionContext,
    ) -> Result<ProvisionResult, Box<dyn std::error::Error + Send + Sync>> {
        let machine_ip = context.endpoint.address.to_string();

        info!(
            playbook = %self.config.playbook.display(),
            machine_ip = %machine_ip,
            "Starting Ansible provisioning"
        );

        // Determine inventory source.
        // If "dynamic", create a temp file with [malbox]\n<machine_ip>.
        let temp_inventory = if self.config.inventory == "dynamic" {
            let content = format!("[malbox]\n{}\n", machine_ip);
            let temp = tempfile::Builder::new()
                .prefix("malbox-ansible-inventory-")
                .suffix(".ini")
                .tempfile()?;
            std::fs::write(temp.path(), &content)?;
            debug!(
                path = %temp.path().display(),
                "Generated dynamic inventory"
            );
            Some(temp)
        } else {
            None
        };

        let inventory_path = match &temp_inventory {
            Some(temp) => temp.path().to_string_lossy().to_string(),
            None => self.config.inventory.clone(),
        };

        // Build the ansible-playbook command.
        let mut cmd = tokio::process::Command::new(&self.config.ansible_bin);
        cmd.arg("-i").arg(&inventory_path);
        cmd.arg(self.config.playbook.as_os_str());

        // Append extra vars if any are configured.
        if !self.config.extra_vars.is_empty() {
            let vars_json = serde_json::to_string(&self.config.extra_vars)?;
            cmd.arg("--extra-vars").arg(&vars_json);
        }

        debug!(command = ?cmd, "Running ansible-playbook");

        // Capture stdout/stderr via pipes so we can still kill the child on timeout.
        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::piped());

        let timeout_duration = std::time::Duration::from_secs(self.config.timeout);
        let mut child = cmd.spawn().map_err(|e| {
            Box::new(AnsibleError::Execution(format!(
                "Failed to spawn ansible-playbook: {}",
                e
            ))) as Box<dyn std::error::Error + Send + Sync>
        })?;

        // Take stdout/stderr handles before waiting.
        let stdout_handle = child.stdout.take();
        let stderr_handle = child.stderr.take();

        match tokio::time::timeout(timeout_duration, child.wait()).await {
            Ok(Ok(status)) => {
                // Read captured output after process exits.
                let stdout = if let Some(mut h) = stdout_handle {
                    let mut buf = Vec::new();
                    tokio::io::AsyncReadExt::read_to_end(&mut h, &mut buf)
                        .await
                        .ok();
                    String::from_utf8_lossy(&buf).into_owned()
                } else {
                    String::new()
                };
                let stderr = if let Some(mut h) = stderr_handle {
                    let mut buf = Vec::new();
                    tokio::io::AsyncReadExt::read_to_end(&mut h, &mut buf)
                        .await
                        .ok();
                    String::from_utf8_lossy(&buf).into_owned()
                } else {
                    String::new()
                };
                let combined = format!("{}{}", stdout, stderr);

                if status.success() {
                    info!("Ansible provisioning completed successfully");
                    Ok(ProvisionResult {
                        status: ProvisionStatus::Success,
                        output: Some(combined),
                    })
                } else {
                    let code = status
                        .code()
                        .map(|c| c.to_string())
                        .unwrap_or_else(|| "unknown".to_string());
                    warn!(exit_code = %code, "Ansible provisioning failed");
                    Ok(ProvisionResult {
                        status: ProvisionStatus::Failed,
                        output: Some(combined),
                    })
                }
            }
            Ok(Err(e)) => {
                warn!(error = %e, "Failed to execute ansible-playbook");
                Err(Box::new(AnsibleError::Execution(format!(
                    "Failed to execute ansible-playbook: {}",
                    e
                ))))
            }
            Err(_) => {
                // Timeout: kill the child process to prevent orphans.
                warn!(
                    timeout_secs = self.config.timeout,
                    "Ansible provisioning timed out, killing child process"
                );
                if let Err(e) = child.kill().await {
                    warn!(error = %e, "Failed to kill timed-out ansible-playbook process");
                }
                Ok(ProvisionResult {
                    status: ProvisionStatus::Failed,
                    output: Some(format!(
                        "Ansible provisioning timed out after {} seconds",
                        self.config.timeout
                    )),
                })
            }
        }
    }

    fn name(&self) -> &str {
        "ansible"
    }
}

// Register with the provisioner registry.
inventory::submit!(ProvisionerMetadata {
    name: "ansible",
    create: |config: &malbox_machinery::provisioner::TomlValue|
        -> Result<Box<dyn Provisioner>, Box<dyn std::error::Error + Send + Sync>> {
        Ok(Box::new(AnsibleProvisioner::new(config)?))
    },
});
