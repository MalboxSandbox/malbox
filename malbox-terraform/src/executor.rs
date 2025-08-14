//! Terraform command execution.
//!
//! This module provides the Terraform command execution functionality,
//! handling process spawning, output capture, and error handling in an
//! async-friendly manner.

use crate::error::{Result, TerraformError};
use crate::output::TerraformOutput;
use malbox_io_utils::process::{AsyncCommand, CommandOutput};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tracing::{debug, info, instrument, trace, warn};

/// Terraform command executor.
///
/// This struct handles the execution of Terraform commands with
/// proper error handling, logging and timeout management.
#[derive(Clone)]
pub struct TerraformExecutor {
    // config: Config,
    terraform_binary: PathBuf,
}

impl TerraformExecutor {
    /// Create a new Terraform executor.
    /// This will also validate that the is available and executable.
    /// TODO: if the binary is not available, prompt for installation.
    pub async fn new(/* TODO: manually set bin path: Config*/) -> Result<Self> {
        let terraform_binary = Self::locate_binary(/*&config*/).await?;

        let executor = Self {
            // config,
            terraform_binary,
        };

        // Validate that the binary exists and is executable.
        executor.validate().await?;

        Ok(executor)
    }

    /// Validate that Terraform is properly installed and executable.
    #[instrument(skip(self))]
    pub async fn validate(&self) -> Result<()> {
        debug!("Validating Terraform installation");

        let output = self
            .run_command::<PathBuf>(&["version".to_string()], None, &HashMap::new())
            .await?;

        if !output.success() {
            return Err(TerraformError::BinaryNotFound {
                message: format!("Terraform version check failed: {}", output.stderr()),
            });
        }

        // NOTE: version checks?
        let version_info = output.stdout();
        info!("Terraform validation sucessful: {}", version_info);

        Ok(())
    }

    /// Intialize a Terraform working directory.
    #[instrument(skip(self, backend_config))]
    pub async fn init<P: AsRef<Path> + std::fmt::Debug>(
        &self,
        working_dir: P,
        backend_config: &HashMap<String, String>,
        variables: &HashMap<String, String>,
    ) -> Result<serde_json::Value> {
        let working_dir = working_dir.as_ref();
        info!("Initializing Terraform in: {}", working_dir.display());

        let mut args = vec!["init".to_string()];

        // Add backend configuration
        for (key, value) in backend_config {
            args.push("-backend-config".to_string());
            args.push(format!("{}={}", key, value));
        }

        let output = self
            .run_command(&args, Some(working_dir), variables)
            .await?;

        if !output.success() {
            return Err(TerraformError::CommandExitCode {
                command: "terraform init".to_string(),
                exit_code: output.exit_code,
                stderr: output.stderr(),
            });
        }

        debug!("Terraform init completed succesfully");
        serde_json::from_str(&output.stdout().trim()).map_err(|e| {
            TerraformError::JsonSerialization {
                message: "Failed to parse JSON output".to_string(),
                source: e,
            }
        })
    }

    /// Plan Terraform changes.
    #[instrument(skip(self, variables))]
    pub async fn plan<P: AsRef<Path> + std::fmt::Debug>(
        &self,
        working_dir: P,
        variables: &HashMap<String, String>,
        target: Option<&str>,
    ) -> Result<serde_json::Value> {
        let working_dir = working_dir.as_ref();
        info!("Planning Terraform changes in: {}", working_dir.display());

        let mut args = vec!["apply".to_string()];

        // Add target if specified
        if let Some(target) = target {
            args.push("-target".to_string());
            args.push(target.to_string());
        }

        let output = self
            .run_command(&args, Some(working_dir), variables)
            .await?;

        if !output.success() {
            return Err(TerraformError::CommandExitCode {
                command: "terraform plan".to_string(),
                exit_code: output.exit_code,
                stderr: output.stderr(),
            });
        }

        debug!("Terraform plan completed succesfully");
        serde_json::from_str(&output.stdout().trim()).map_err(|e| {
            TerraformError::JsonSerialization {
                message: "Failed to parse JSON output".to_string(),
                source: e,
            }
        })
    }

    /// Apply Terraform configuration.
    #[instrument(skip(self, variables))]
    pub async fn apply<P: AsRef<Path> + std::fmt::Debug>(
        &self,
        working_dir: P,
        variables: &HashMap<String, String>,
        target: Option<&str>,
        auto_approve: bool,
    ) -> Result<serde_json::Value> {
        let working_dir = working_dir.as_ref();
        info!(
            "Applying Terraform configuration in: {}",
            working_dir.display()
        );

        let mut args = vec!["apply".to_string()];

        if auto_approve {
            args.push("-auto-approve".to_string());
        }

        // Add target if specified
        if let Some(target) = target {
            args.push("-target".to_string());
            args.push(target.to_string());
        }

        let output = self
            .run_command(&args, Some(working_dir), variables)
            .await?;

        if !output.success() {
            return Err(TerraformError::CommandExitCode {
                command: "terraform apply".to_string(),
                exit_code: output.exit_code,
                stderr: output.stderr(),
            });
        }

        info!("Terraform apply completed successfully");
        serde_json::from_str(&output.stdout().trim()).map_err(|e| {
            TerraformError::JsonSerialization {
                message: "Failed to parse JSON output".to_string(),
                source: e,
            }
        })
    }

    /// Destroy Terraform-managed infrastructure.
    #[instrument(skip(self, variables))]
    pub async fn destroy<P: AsRef<Path> + std::fmt::Debug>(
        &self,
        working_dir: P,
        variables: &HashMap<String, String>,
        target: Option<&str>,
        auto_approve: bool,
    ) -> Result<serde_json::Value> {
        let working_dir = working_dir.as_ref();
        warn!(
            "Destroying Terraform infrastructure in: {}",
            working_dir.display()
        );

        let mut args = vec!["destroy".to_string()];

        if auto_approve {
            args.push("-auto-approve".to_string());
        }

        if let Some(target) = target {
            args.push("-target".to_string());
            args.push(target.to_string());
        }

        let output = self
            .run_command(&args, Some(working_dir), variables)
            .await?;

        if !output.success() {
            return Err(TerraformError::CommandExitCode {
                command: "terraform destroy".to_string(),
                exit_code: output.exit_code,
                stderr: output.stderr(),
            });
        }

        info!("Terraform destroy completed successfully");
        serde_json::from_str(&output.stdout().trim()).map_err(|e| {
            TerraformError::JsonSerialization {
                message: "Failed to parse JSON output".to_string(),
                source: e,
            }
        })
    }

    /// Get Terraform output values.
    #[instrument(skip(self))]
    pub async fn output<P: AsRef<Path> + std::fmt::Debug>(
        &self,
        working_dir: P,
        output_name: Option<&str>,
    ) -> Result<serde_json::Value> {
        let working_dir = working_dir.as_ref();
        debug!("Getting terraform output from: {}", working_dir.display());

        let mut args = vec!["output".to_string()];

        if let Some(name) = output_name {
            args.push(name.to_string());
        }

        let output = self
            .run_command(&args, Some(working_dir), &HashMap::new())
            .await?;

        if !output.success() {
            return Err(TerraformError::CommandExitCode {
                command: "terraform output".to_string(),
                exit_code: output.exit_code,
                stderr: output.stderr(),
            });
        }

        info!("Terraform output retrieved successfully");
        serde_json::from_str(&output.stdout().trim()).map_err(|e| {
            TerraformError::JsonSerialization {
                message: "Failed to parse JSON output".to_string(),
                source: e,
            }
        })
    }

    /// Show current state or plan.
    #[instrument(skip(self))]
    pub async fn show<P: AsRef<Path> + std::fmt::Debug>(
        &self,
        working_dir: P,
    ) -> Result<serde_json::Value> {
        let working_dir = working_dir.as_ref();
        debug!(
            "Showing Terraform state/plan from: {}",
            working_dir.display()
        );

        let args = vec!["show".to_string()];

        let output = self
            .run_command(&args, Some(working_dir), &HashMap::new())
            .await?;

        if !output.success() {
            return Err(TerraformError::CommandExitCode {
                command: "terraform show".to_string(),
                exit_code: output.exit_code,
                stderr: output.stderr(),
            });
        }

        info!("Terraform show completed successfully");
        serde_json::from_str(&output.stdout().trim()).map_err(|e| {
            TerraformError::JsonSerialization {
                message: "Failed to parse JSON output".to_string(),
                source: e,
            }
        })
    }

    /// Import existing infrastructure into Terraform state.
    #[instrument(skip(self, variables))]
    pub async fn import<P: AsRef<Path> + std::fmt::Debug>(
        &self,
        working_dir: P,
        address: &str,
        id: &str,
        variables: &HashMap<String, String>,
    ) -> Result<serde_json::Value> {
        let working_dir = working_dir.as_ref();
        info!(
            "Importing resource {id} as {address} in: {}",
            working_dir.display()
        );

        let args = vec!["import".to_string(), address.to_string(), id.to_string()];

        let output = self
            .run_command(&args, Some(working_dir), variables)
            .await?;

        if !output.success() {
            return Err(TerraformError::CommandExitCode {
                command: format!("terraform import {} {}", address, id),
                exit_code: output.exit_code,
                stderr: output.stderr(),
            });
        }

        info!("Terraform import completed successfully");
        serde_json::from_str(&output.stdout().trim()).map_err(|e| {
            TerraformError::JsonSerialization {
                message: "Failed to parse JSON output".to_string(),
                source: e,
            }
        })
    }

    /// Run a raw Terraform command with custom arguments.
    #[instrument(skip(self, variables))]
    pub async fn run_command<P: AsRef<Path> + std::fmt::Debug>(
        &self,
        args: &[String],
        working_dir: Option<P>,
        variables: &HashMap<String, String>,
    ) -> Result<CommandOutput> {
        let mut cmd = AsyncCommand::new(&self.terraform_binary);

        // Add standard flags for machine-readable output
        cmd = cmd.arg("-machine-readable");
        cmd = cmd.arg("-color=false");
        cmd = cmd.arg("-timestamp-ui");

        // Add arguments
        cmd = cmd.args(args);

        // Add variable as -var arguments
        for (key, value) in variables {
            cmd = cmd.arg("-var").arg(format!("{}={}", key, value));
        }

        // Set working directory if provided
        if let Some(dir) = working_dir {
            cmd = cmd.current_dir(dir.as_ref());
        }

        // Set environment variables
        //
        // By setting `TF_IN_AUTOMATION` to any non-empty value, Terraform adjusts its output
        // to avoid suggesting specific commands to run next. In our case, we want consistent outputs, hence
        // we set it to `1`.
        cmd = cmd.env("TF_IN_AUTOMATION", "1");
        // By setting `TF_INPUT` to `0`, Terraform will behave as if the `-input=false` was specified.
        // We use this to disable prompts for variables that haven't had their values specified.
        cmd = cmd.env("TF_INPUT", "0");

        let command_str = format!("terraform {}", args.join(" "));
        trace!("Executing: {}", command_str);

        // TODO: Maybe optional timeout?
        // e.g. with config setting, let operation_timeout = Duration::from_secs(self.config.operation_timeout_seconds);

        let result = cmd.run_with_standard_logging().await;

        match result {
            Ok(output) => {
                trace!("Command completed with exit code: {}", output.exit_code);
                Ok(output)
            }
            Err(e) => Err(TerraformError::CommandFailed {
                command: command_str,
                source: e,
            }),
        }
    }
    /// Locate the Terraform binary.
    async fn locate_binary(/*config: &Config*/) -> Result<PathBuf> {
        // Use explicit path if provided
        //  if let Some(binary) = &config.terraform_binary {
        //         if binary.exists() {
        //             return Ok(binary.clone());
        //          } else {
        //             return Err(TerraformError::BinaryNotFound {
        //                 message: format!("Terraform binary not found at: {}", binary.display()),
        //             });
        //         }
        //     }
        // }
        //

        // Try to find in PATH
        let output = AsyncCommand::new("which")
            .arg("terraform")
            .run()
            .await
            .map_err(|_| TerraformError::BinaryNotFound {
                message: "Terraform binary not found in PATH".to_string(),
            })?;

        if output.success() {
            let stdout = output.stdout();
            let path = stdout.trim();
            Ok(PathBuf::from(path))
        } else {
            Err(TerraformError::BinaryNotFound {
                message: "Terraform binary not found in PATH".to_string(),
            })
        }
    }
}
