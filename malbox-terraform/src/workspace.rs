//! Terraform workspace management.
//!
//! This module provides functionality for managing Terraform workspaces,
//! including creation, selection, deletion, and isolation of different
//! environments and resource sets.

use crate::{
    error::{Result, TerraformError},
    executor::TerraformExecutor,
};
use malbox_config::machinery::vmware::NetworkConfig;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tracing::{debug, info, instrument, warn};

/// Terraform workspace manager.
///
/// Handles workspace lifecycle operations including creatin, selection,
/// and deletion. Workspaces provide isolation between different environments
/// and resource sets.
#[derive(Clone)]
pub struct WorkspaceManager {
    // config: Config,
}

impl WorkspaceManager {
    /// Create a new workspace manager.
    pub fn new(/*config: Config*/) -> Self {
        Self {
            // config
        }
    }

    /// Create a new workspace.
    #[instrument(skip(self, executor))]
    pub async fn create_workspace<P: AsRef<Path> + std::fmt::Debug>(
        &self,
        executor: &TerraformExecutor,
        working_dir: P,
        workspace_name: &str,
    ) -> Result<WorkspaceInfo> {
        let working_dir = working_dir.as_ref();
        info!(
            "Creating workspace '{workspace_name}' in: {}",
            working_dir.display()
        );

        // Check if workspace already exists
        if self
            .workspace_exists(executor, working_dir, workspace_name)
            .await?
        {
            return Err(TerraformError::WorkspaceError {
                operation: "create".to_string(),
                message: format!("Workspace '{}' already exists", workspace_name),
            });
        }

        let args = vec![
            "workspace".to_string(),
            "new".to_string(),
            workspace_name.to_string(),
        ];
        let output = executor
            .run_command(&args, Some(working_dir), &HashMap::new())
            .await?;

        if !output.success() {
            return Err(TerraformError::CommandExitCode {
                command: format!("terraform workspace new {}", workspace_name),
                exit_code: output.exit_code,
                stderr: output.stderr(),
            });
        }

        info!("Successfully created workspace: {}", workspace_name);
        Ok(WorkspaceInfo {
            name: workspace_name.to_string(),
            current: true,
            working_dir: working_dir.to_path_buf(),
        })
    }

    /// Select an existing workspace.
    #[instrument(skip(self, executor))]
    pub async fn select_workspace<P: AsRef<Path> + std::fmt::Debug>(
        &self,
        executor: &TerraformExecutor,
        working_dir: P,
        workspace_name: &str,
    ) -> Result<WorkspaceInfo> {
        let working_dir = working_dir.as_ref();
        info!(
            "Selecting workspace '{workspace_name}' in: {}",
            working_dir.display()
        );

        // Check if workspace exists
        if !self
            .workspace_exists(executor, working_dir, workspace_name)
            .await?
        {
            return Err(TerraformError::WorkspaceError {
                operation: "select".to_string(),
                message: format!("Workspace '{workspace_name}' does not exist"),
            });
        }

        let args = vec![
            "workspace".to_string(),
            "select".to_string(),
            workspace_name.to_string(),
        ];
        let output = executor
            .run_command(&args, Some(working_dir), &HashMap::new())
            .await?;

        if !output.success() {
            return Err(TerraformError::CommandExitCode {
                command: format!("terraform workspace select {workspace_name}"),
                exit_code: output.exit_code,
                stderr: output.stderr(),
            });
        }

        debug!("Successfully selected workspace: {workspace_name}");

        Ok(WorkspaceInfo {
            name: workspace_name.to_string(),
            current: true,
            working_dir: working_dir.to_path_buf(),
        })
    }

    /// Delete a workspace.
    #[instrument(skip(self, executor))]
    pub async fn delete_workspace<P: AsRef<Path> + std::fmt::Debug>(
        &self,
        executor: &TerraformExecutor,
        working_dir: P,
        workspace_name: &str,
        force: bool,
    ) -> Result<()> {
        let working_dir = working_dir.as_ref();
        warn!(
            "Deleting workspace '{workspace_name} in: {}",
            working_dir.display()
        );

        // Cannot delete the default workspace
        if workspace_name == "default" {
            return Err(TerraformError::WorkspaceError {
                operation: "delete".to_string(),
                message: "Cannot delete the default workspace".to_string(),
            });
        }

        // Check if workspace exists
        if !self
            .workspace_exists(executor, working_dir, workspace_name)
            .await?
        {
            return Err(TerraformError::WorkspaceError {
                operation: "delete".to_string(),
                message: format!("Workspace '{workspace_name}' does not exist"),
            });
        }

        // Switch to default workspace first
        self.select_workspace(executor, working_dir, "default")
            .await?;

        let mut args = vec!["workspace".to_string(), "delete".to_string()];
        if force {
            args.push("-force".to_string());
        }
        args.push(workspace_name.to_string());

        let output = executor
            .run_command(&args, Some(working_dir), &HashMap::new())
            .await?;

        if !output.success() {
            return Err(TerraformError::CommandExitCode {
                command: format!("terraform workspace delete {workspace_name}"),
                exit_code: output.exit_code,
                stderr: output.stderr(),
            });
        }

        info!("Successfully delete workspace: {}", workspace_name);
        Ok(())
    }

    /// List all workspaces.
    #[instrument(skip(self, executor))]
    pub async fn list_workspaces<P: AsRef<Path> + std::fmt::Debug>(
        &self,
        executor: &TerraformExecutor,
        working_dir: P,
    ) -> Result<Vec<WorkspaceInfo>> {
        let working_dir = working_dir.as_ref();
        debug!("Listing workspaces in: {}", working_dir.display());

        let args = vec!["workspace".to_string(), "list".to_string()];
        let output = executor
            .run_command(&args, Some(working_dir), &HashMap::new())
            .await?;

        if !output.success() {
            return Err(TerraformError::CommandExitCode {
                command: "terraform workspace list".to_string(),
                exit_code: output.exit_code,
                stderr: output.stderr(),
            });
        }

        let mut workspaces = Vec::new();

        // NOTE: Since the `-json` flag is not available for this command,
        // we'll need to parse the user-readable output directly.
        for line in output.stdout().lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }

            // Current workspace name is marked with `*`
            let (current, name) = if line.starts_with("* ") {
                (true, &line[2..])
            } else if line.starts_with(" ") {
                (false, &line[2..])
            } else {
                (false, line)
            };

            workspaces.push(WorkspaceInfo {
                name: name.to_string(),
                current,
                working_dir: working_dir.to_path_buf(),
            });
        }

        info!("Found {} workspaces", workspaces.len());
        Ok(workspaces)
    }

    /// Get the current workspace.
    #[instrument(skip(self, executor))]
    pub async fn current_workspace<P: AsRef<Path> + std::fmt::Debug>(
        &self,
        executor: &TerraformExecutor,
        working_dir: P,
    ) -> Result<WorkspaceInfo> {
        let working_dir = working_dir.as_ref();

        let workspaces = self.list_workspaces(executor, working_dir).await?;

        for workspace in workspaces {
            if workspace.current {
                return Ok(workspace);
            }
        }

        // Fallback to default if no current workspace found
        Ok(WorkspaceInfo {
            name: "default".to_string(),
            current: true,
            working_dir: working_dir.to_path_buf(),
        })
    }

    /// Check if a workspace exists.
    #[instrument(skip(self, executor))]
    pub async fn workspace_exists<P: AsRef<Path> + std::fmt::Debug>(
        &self,
        executor: &TerraformExecutor,
        working_dir: P,
        workspace_name: &str,
    ) -> Result<bool> {
        let workspaces = self.list_workspaces(executor, working_dir).await?;

        Ok(workspaces.iter().any(|w| w.name == workspace_name))
    }

    /// Ensure a workspace exists, creating if necessary.
    #[instrument(skip(self, executor))]
    pub async fn ensure_workspace<P: AsRef<Path> + std::fmt::Debug>(
        &self,
        executor: &TerraformExecutor,
        working_dir: P,
        workspace_name: &str,
    ) -> Result<WorkspaceInfo> {
        let working_dir = working_dir.as_ref();

        if self
            .workspace_exists(executor, working_dir, workspace_name)
            .await?
        {
            self.select_workspace(executor, working_dir, workspace_name)
                .await
        } else {
            self.create_workspace(executor, working_dir, workspace_name)
                .await
        }
    }

    // /// Create a workspace configuration for a specific environment.
    //     pub fn create_environment_config<S: AsRef<str>>(
    //         &self,
    //         environment: S,
    //         variables: HashMap<String, String>,
    //     ) -> WorkspaceConfig {
    //         todo!()
    //     }
}

/// Information about a Terraform workspace.
pub struct WorkspaceInfo {
    /// Name of the workspace.
    pub name: String,
    /// Whether this is the currently selected workspace.
    pub current: bool,
    /// Working directory for this workspace.
    pub working_dir: PathBuf,
}

impl WorkspaceInfo {
    /// Get the state file path for this workspace.
    pub fn state_file_path(&self) -> PathBuf {
        if self.name == "default" {
            self.working_dir.join("terraform.tfstate")
        } else {
            self.working_dir
                .join("terraform.tfstate.d")
                .join(&self.name)
                .join("terraform.tfstate")
        }
    }
}
