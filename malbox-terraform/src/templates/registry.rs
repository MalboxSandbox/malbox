//! Template registry and management.
//!
//! This module provides template discovery, validation and management
//! functionality. It automatically discovers user-provided Terraform
//! templates and maintains metadata about their capabilities and requirements.

use crate::error::{Result, TerraformError};
use crate::utils::parser::{TemplateInfo, parse_template};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use tracing::{debug, warn};

/// Template registry entry with metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateEntry {
    pub name: String,
    pub path: PathBuf,
    pub info: TemplateInfo,
    pub version: Option<String>,
    pub author: Option<String>,
    pub description: Option<String>,
}

/// Template registry for managing Terraform templates.
#[derive(Debug, Default)]
pub struct TemplateRegistry {
    templates: HashMap<String, TemplateEntry>,
}

impl TemplateRegistry {
    /// Create a new template registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Discover templates in a directory.
    pub fn discover_templates(&mut self, templates_dir: &Path) -> Result<usize> {
        debug!("Discovering templates in: {:?}", templates_dir);

        if !templates_dir.exists() {
            return Ok(0);
        }

        let mut discovered = 0;

        for entry in fs::read_dir(templates_dir).map_err(|e| TerraformError::Io {
            operation: format!("read directory {}", templates_dir.display()),
            source: e,
        })? {
            let entry = entry.map_err(|e| TerraformError::Io {
                operation: "read directory entry".to_string(),
                source: e,
            })?;

            let path = entry.path();
            if path.is_dir() {
                if let Ok(count) = self.discover_in_directory(&path) {
                    discovered += count;
                }
            }
        }

        debug!("Discovered {} templates", discovered);
        Ok(discovered)
    }

    /// Discover templates in a specific directory.
    fn discover_in_directory(&mut self, dir: &Path) -> Result<usize> {
        let mut count = 0;

        // Look for main.tf or *.tf files
        for entry in fs::read_dir(dir).map_err(|e| TerraformError::Io {
            operation: format!("read directory {}", dir.display()),
            source: e,
        })? {
            let entry = entry.map_err(|e| TerraformError::Io {
                operation: "read directory entry".to_string(),
                source: e,
            })?;

            let path = entry.path();
            if path.extension().map_or(false, |ext| ext == "tf") {
                match self.register_template(&path) {
                    Ok(_) => count += 1,
                    Err(e) => warn!("Failed to register template {:?}: {}", path, e),
                }
                break; // Only process one .tf file per directory
            }
        }

        Ok(count)
    }

    /// Register a single template.
    pub fn register_template(&mut self, template_path: &Path) -> Result<()> {
        debug!("Registering template: {:?}", template_path);

        let info = parse_template(template_path)?;
        let name = template_path
            .parent()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();

        let entry = TemplateEntry {
            name: name.clone(),
            path: template_path.to_path_buf(),
            info,
            version: None,
            author: None,
            description: None,
        };

        self.templates.insert(name, entry);
        Ok(())
    }

    /// Get all registered templates.
    pub fn list_templates(&self) -> Vec<&TemplateEntry> {
        self.templates.values().collect()
    }

    /// Get a specific template by name.
    pub fn get_template(&self, name: &str) -> Option<&TemplateEntry> {
        self.templates.get(name)
    }

    /// Validate a template's requirements.
    pub fn validate_template(&self, name: &str, variables: &HashMap<String, String>) -> Result<()> {
        let template = self
            .get_template(name)
            .ok_or_else(|| TerraformError::ConfigValidation {
                message: format!("Template '{}' not found", name),
            })?;

        // Check required variables
        for (var_name, var_info) in &template.info.variables {
            if !var_info.has_default && !variables.contains_key(var_name) {
                return Err(TerraformError::MissingVariable {
                    variable: var_name.clone(),
                });
            }
        }

        Ok(())
    }

    /// Get template requirements.
    pub fn get_requirements(&self, name: &str) -> Result<TemplateRequirements> {
        let template = self
            .get_template(name)
            .ok_or_else(|| TerraformError::ConfigValidation {
                message: format!("Template '{}' not found", name),
            })?;

        let required_variables: Vec<String> = template
            .info
            .variables
            .iter()
            .filter(|(_, var)| !var.has_default)
            .map(|(name, _)| name.clone())
            .collect();

        let providers: Vec<String> = template.info.required_providers.keys().cloned().collect();

        Ok(TemplateRequirements {
            required_variables,
            providers,
            resource_count: template.info.resources.len(),
        })
    }

    /// Clear all registered templates.
    pub fn clear(&mut self) {
        self.templates.clear();
    }
}

/// Template requirements summary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateRequirements {
    pub required_variables: Vec<String>,
    pub providers: Vec<String>,
    pub resource_count: usize,
}
