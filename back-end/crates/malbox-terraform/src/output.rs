//! Terraform output processing and normalization.
//!
//! This module provides unified output processing for Terraform commands,
//! normalizing both JSON and machine-readable formats into consistent
//! user-friendly output.

use crate::error::{Result, TerraformError};
use serde::{Deserialize, Serialize};

/// Unified output format for Terraform commands.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerraformOutput {
    pub success: bool,
    pub message: String,
    pub details: Option<serde_json::Value>,
    pub resources_changed: Option<ResourceChanges>,
}

/// Resource change summary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceChanges {
    pub add: u32,
    pub change: u32,
    pub destroy: u32,
}

impl TerraformOutput {
    /// Create output from JSON response.
    pub fn from_json(json_str: &str, success: bool) -> Self {
        if success {
            match serde_json::from_str::<serde_json::Value>(json_str) {
                Ok(json_data) => Self {
                    success,
                    message: Self::extract_message_from_json(&json_data),
                    details: Some(json_data.clone()),
                    resources_changed: Self::extract_resource_changes_from_json(&json_data),
                },
                Err(_) => Self {
                    success,
                    message: json_str.to_string(),
                    details: None,
                    resources_changed: None,
                },
            }
        } else {
            Self {
                success,
                message: json_str.to_string(),
                details: None,
                resources_changed: None,
            }
        }
    }

    /// Create output from machine-readable response.
    pub fn from_machine_readable(output: &str, success: bool) -> Self {
        let message = Self::parse_machine_readable_message(output);
        let resources_changed = Self::parse_machine_readable_resources(output);

        Self {
            success,
            message,
            details: None,
            resources_changed,
        }
    }

    /// Extract user-friendly message from JSON.
    fn extract_message_from_json(json: &serde_json::Value) -> String {
        // Try to extract meaningful message from different JSON structures
        if let Some(message) = json.get("message").and_then(|m| m.as_str()) {
            return message.to_string();
        }

        if let Some(changes) = json.get("resource_changes").and_then(|c| c.as_array()) {
            return format!("Plan generated with {} resource changes", changes.len());
        }

        "Operation completed".to_string()
    }

    /// Extract resource changes from JSON.
    fn extract_resource_changes_from_json(json: &serde_json::Value) -> Option<ResourceChanges> {
        json.get("resource_changes")
            .and_then(|changes| changes.as_array())
            .map(|changes| {
                let mut add = 0;
                let mut change = 0;
                let mut destroy = 0;

                for change_obj in changes {
                    if let Some(actions) = change_obj
                        .get("change")
                        .and_then(|c| c.get("actions"))
                        .and_then(|a| a.as_array())
                    {
                        for action in actions {
                            if let Some(action_str) = action.as_str() {
                                match action_str {
                                    "create" => add += 1,
                                    "update" => change += 1,
                                    "delete" => destroy += 1,
                                    _ => {}
                                }
                            }
                        }
                    }
                }

                ResourceChanges {
                    add,
                    change,
                    destroy,
                }
            })
    }

    /// Parse message from machine-readable output.
    fn parse_machine_readable_message(output: &str) -> String {
        for line in output.lines() {
            let parts: Vec<&str> = line.split(',').collect();
            if parts.len() >= 4 && parts[2] == "ui" && parts[3] == "output" {
                if let Some(message) = parts.get(4) {
                    return message.replace("%!(PACKER_COMMA)", ",");
                }
            }
        }
        "Operation completed".to_string()
    }

    /// Parse resource changes from machine-readable output.
    fn parse_machine_readable_resources(output: &str) -> Option<ResourceChanges> {
        for line in output.lines() {
            let parts: Vec<&str> = line.split(',').collect();
            if parts.len() >= 4 && parts[2] == "ui" && parts[3] == "output" {
                if let Some(message) = parts.get(4) {
                    if message.contains("Resources:") {
                        // Parse patterns like "Resources: 1 added, 0 changed, 0 destroyed"
                        let mut add = 0;
                        let mut change = 0;
                        let mut destroy = 0;

                        if let Some(added) = extract_number_before(message, "added") {
                            add = added;
                        }
                        if let Some(changed) = extract_number_before(message, "changed") {
                            change = changed;
                        }
                        if let Some(destroyed) = extract_number_before(message, "destroyed") {
                            destroy = destroyed;
                        }

                        return Some(ResourceChanges {
                            add,
                            change,
                            destroy,
                        });
                    }
                }
            }
        }
        None
    }
}

/// Helper function to extract numbers from machine-readable text.
fn extract_number_before(text: &str, keyword: &str) -> Option<u32> {
    if let Some(pos) = text.find(keyword) {
        let before = &text[..pos];
        if let Some(number_start) = before.rfind(' ') {
            if let Ok(num) = before[number_start + 1..].trim().parse::<u32>() {
                return Some(num);
            }
        }
    }
    None
}

/// Terraform machine-readable event types.
#[derive(Debug)]
pub enum TerraformEvent {
    UiMessage {
        message_type: String,
        message: String,
    },
    ResourceChange {
        resource: String,
        action: String,
    },
    DiagnosticMessage {
        severity: String,
        summary: String,
        detail: Option<String>,
    },
    PlanSummary {
        add: u32,
        change: u32,
        destroy: u32,
    },
    Generic {
        event_type: String,
        data: String,
    },
}

/// Parse terraform machine-readable event.
pub fn parse_terraform_event(content: &str) -> Result<TerraformEvent> {
    // Terraform machine-readable format: timestamp,log_level,message_type,hook_data...
    let parts: Vec<&str> = content.splitn(4, ',').collect();
    if parts.len() < 3 {
        return Err(TerraformError::Parsing(
            "Invalid machine-readable format".to_string(),
        ));
    }

    let _timestamp = parts[0];
    let _log_level = parts[1];
    let message_type = parts[2];
    let data = parts.get(3).unwrap_or(&"");

    match message_type {
        "ui" => {
            let ui_parts: Vec<&str> = data.splitn(2, ',').collect();
            let ui_type = ui_parts[0];
            let message = ui_parts.get(1).unwrap_or(&"").to_string();

            Ok(TerraformEvent::UiMessage {
                message_type: ui_type.to_string(),
                message,
            })
        }
        "resource_change" => {
            // Parse resource change data
            let change_parts: Vec<&str> = data.splitn(3, ',').collect();
            let resource = change_parts.get(0).unwrap_or(&"").to_string();
            let action = change_parts.get(1).unwrap_or(&"").to_string();

            Ok(TerraformEvent::ResourceChange { resource, action })
        }
        "diagnostic" => {
            let diag_parts: Vec<&str> = data.splitn(3, ',').collect();
            let severity = diag_parts.get(0).unwrap_or(&"").to_string();
            let summary = diag_parts.get(1).unwrap_or(&"").to_string();
            let detail = diag_parts.get(2).map(|s| s.to_string());

            Ok(TerraformEvent::DiagnosticMessage {
                severity,
                summary,
                detail,
            })
        }
        "plan_summary" => {
            let summary_parts: Vec<&str> = data.splitn(3, ',').collect();
            let add = summary_parts.get(0).unwrap_or(&"0").parse().unwrap_or(0);
            let change = summary_parts.get(1).unwrap_or(&"0").parse().unwrap_or(0);
            let destroy = summary_parts.get(2).unwrap_or(&"0").parse().unwrap_or(0);

            Ok(TerraformEvent::PlanSummary {
                add,
                change,
                destroy,
            })
        }
        _ => Ok(TerraformEvent::Generic {
            event_type: message_type.to_string(),
            data: data.to_string(),
        }),
    }
}

/// Log terraform event with proper formatting.
pub fn log_terraform_event(event: &TerraformEvent) {
    use console::style;

    match event {
        TerraformEvent::UiMessage {
            message_type,
            message,
        } => match message_type.as_str() {
            "output" => println!("{}", message),
            "error" => println!("{} {}", style("Error:").red().bold(), message),
            "warning" => println!("{} {}", style("Warning:").yellow().bold(), message),
            _ => println!("{}", message),
        },
        TerraformEvent::ResourceChange { resource, action } => {
            let action_style = match action.as_str() {
                "create" => style("+").green().bold(),
                "update" => style("~").yellow().bold(),
                "delete" => style("-").red().bold(),
                _ => style("•").dim(),
            };
            println!("  {} {}", action_style, resource);
        }
        TerraformEvent::DiagnosticMessage {
            severity,
            summary,
            detail,
        } => {
            let severity_style = match severity.as_str() {
                "error" => style("Error:").red().bold(),
                "warning" => style("Warning:").yellow().bold(),
                _ => style("Info:").blue().bold(),
            };
            println!("{} {}", severity_style, summary);
            if let Some(detail_text) = detail {
                println!("  {}", style(detail_text).dim());
            }
        }
        TerraformEvent::PlanSummary {
            add,
            change,
            destroy,
        } => {
            println!(
                "{} Plan: {} to add, {} to change, {} to destroy",
                style("»").bold().blue(),
                style(add).green(),
                style(change).yellow(),
                style(destroy).red()
            );
        }
        TerraformEvent::Generic { event_type, data } => {
            if !data.is_empty() {
                println!("  {}: {}", style(event_type).dim(), data);
            }
        }
    }
}
