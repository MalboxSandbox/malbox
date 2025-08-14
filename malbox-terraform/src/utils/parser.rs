//! Parsing utilities for Terraform-related files.

use crate::error::{Result, TerraformError};
use hcl::{Block, Body, Expression};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use tracing::debug;

/// Terraform variable definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Variable {
    pub name: String,
    pub description: Option<String>,
    pub type_constraint: Option<String>,
    pub has_default: bool,
    pub sensitive: bool,
}

/// Terraform resource definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Resource {
    pub resource_type: String,
    pub name: String,
}

/// Terraform provider configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Provider {
    pub name: String,
    pub version_constraint: Option<String>,
}

/// Terraform output definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Output {
    pub name: String,
    pub description: Option<String>,
    pub sensitive: bool,
}

/// Complete Terraform template analysis.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateInfo {
    pub variables: HashMap<String, Variable>,
    pub resources: Vec<Resource>,
    pub providers: Vec<Provider>,
    pub outputs: HashMap<String, Output>,
    pub required_providers: HashMap<String, String>,
}

/// Parse Terraform file for complete analysis.
pub fn parse_template(path: &Path) -> Result<TemplateInfo> {
    debug!("Parsing Terraform template: {:?}", path);

    let content = std::fs::read_to_string(path).map_err(|e| TerraformError::Io {
        operation: format!("read file {}", path.display()),
        source: e,
    })?;

    let body = hcl::parse(&content).map_err(|e| TerraformError::HclParsing {
        message: format!("Failed to parse {}", path.display()),
        source: e,
    })?;

    analyze_template(&body)
}

/// Analyze parsed HCL body for complete template information.
fn analyze_template(body: &Body) -> Result<TemplateInfo> {
    let mut variables = HashMap::new();
    let mut resources = Vec::new();
    let mut providers = Vec::new();
    let mut outputs = HashMap::new();
    let mut required_providers = HashMap::new();

    for block in body.blocks() {
        match block.identifier.as_str() {
            "variable" => {
                if let Some(var_name) = block.labels.first() {
                    let var = parse_variable(var_name.as_str(), block);
                    variables.insert(var_name.as_str().to_string(), var);
                }
            }
            "resource" => {
                if block.labels.len() >= 2 {
                    let resource = Resource {
                        resource_type: block.labels[0].as_str().to_string(),
                        name: block.labels[1].as_str().to_string(),
                    };
                    resources.push(resource);
                }
            }
            "provider" => {
                if let Some(provider_name) = block.labels.first() {
                    let provider = parse_provider(provider_name.as_str(), block);
                    providers.push(provider);
                }
            }
            "output" => {
                if let Some(output_name) = block.labels.first() {
                    let output = parse_output(output_name.as_str(), block);
                    outputs.insert(output_name.as_str().to_string(), output);
                }
            }
            "terraform" => {
                // Extract required_providers from terraform block
                for nested_block in block.body.blocks() {
                    if nested_block.identifier.as_str() == "required_providers" {
                        for attr in nested_block.body.attributes() {
                            required_providers
                                .insert(attr.key().to_string(), attr.expr().to_string());
                        }
                    }
                }
            }
            _ => {} // Ignore other blocks
        }
    }

    Ok(TemplateInfo {
        variables,
        resources,
        providers,
        outputs,
        required_providers,
    })
}

/// Parse variable block.
fn parse_variable(name: &str, block: &Block) -> Variable {
    let mut var = Variable {
        name: name.to_string(),
        description: None,
        type_constraint: None,
        has_default: false,
        sensitive: false,
    };

    for attr in block.body.attributes() {
        match attr.key() {
            "description" => {
                var.description = extract_string_literal(attr.expr());
            }
            "type" => {
                var.type_constraint = Some(attr.expr().to_string());
            }
            "default" => {
                var.has_default = true;
            }
            "sensitive" => {
                var.sensitive = extract_bool_literal(attr.expr()).unwrap_or(false);
            }
            _ => {}
        }
    }

    var
}

/// Parse provider block.
fn parse_provider(name: &str, block: &Block) -> Provider {
    let mut provider = Provider {
        name: name.to_string(),
        version_constraint: None,
    };

    for attr in block.body.attributes() {
        if attr.key() == "version" {
            provider.version_constraint = extract_string_literal(attr.expr());
        }
    }

    provider
}

/// Parse output block.
fn parse_output(name: &str, block: &Block) -> Output {
    let mut output = Output {
        name: name.to_string(),
        description: None,
        sensitive: false,
    };

    for attr in block.body.attributes() {
        match attr.key() {
            "description" => {
                output.description = extract_string_literal(attr.expr());
            }
            "sensitive" => {
                output.sensitive = extract_bool_literal(attr.expr()).unwrap_or(false);
            }
            _ => {}
        }
    }

    output
}

/// Extract string literal from expression.
fn extract_string_literal(expr: &Expression) -> Option<String> {
    match expr {
        Expression::String(s) => Some(s.clone()),
        _ => None,
    }
}

/// Extract boolean literal from expression.
fn extract_bool_literal(expr: &Expression) -> Option<bool> {
    match expr {
        Expression::Bool(b) => Some(*b),
        _ => None,
    }
}

/// Parse .tfvars file for variable values.
pub fn parse_tfvars(path: &Path) -> Result<HashMap<String, String>> {
    debug!("Parsing tfvars file: {:?}", path);

    let content = std::fs::read_to_string(path).map_err(|e| TerraformError::Io {
        operation: format!("read file {}", path.display()),
        source: e,
    })?;

    let body = hcl::parse(&content).map_err(|e| TerraformError::HclParsing {
        message: format!("Failed to parse {}", path.display()),
        source: e,
    })?;

    let mut vars = HashMap::new();

    for attr in body.attributes() {
        if let Some(value) = extract_string_literal(attr.expr()) {
            vars.insert(attr.key().to_string(), value);
        } else {
            vars.insert(attr.key().to_string(), attr.expr().to_string());
        }
    }

    Ok(vars)
}
