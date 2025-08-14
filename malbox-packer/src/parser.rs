use crate::error::{Error, Result};
use crate::templates::vars::VarType;
use crate::templates::{Provisioner, Source, Template, TemplateDependencies, Variable};
use hcl::{Block, Body, Expression, Value};
use std::collections::{HashMap, HashSet};
use std::path::Path;

pub fn parse_packer_event(content: &str) -> Result<PackerEvent> {
    // Packer uses %!(PACKER_COMMA) as a placeholder for commas in the data portion
    // We need to split first, then replace in the data portion only

    // Machine-readable format: timestamp,target,type,data...
    let parts: Vec<&str> = content.splitn(4, ',').collect();
    if parts.len() < 3 {
        return Err(Error::Parsing(
            "Invalid machine-readable format".to_string(),
        ));
    }

    let _timestamp = parts[0];
    let target = parts[1];
    let event_type = parts[2];
    let data = parts.get(3).unwrap_or(&"");

    // Now we can safely replace the comma placeholder in the data portion
    let data = data.replace("%!(PACKER_COMMA)", ",");

    match event_type {
        "ui" => {
            let ui_parts: Vec<&str> = data.splitn(2, ',').collect();
            let ui_type = ui_parts[0];
            let message = ui_parts
                .get(1)
                .unwrap_or(&"")
                .replace("\\n", "\n")
                .replace("\\\"", "\"")
                .to_string();

            Ok(PackerEvent::UiMessage {
                target: target.to_string(),
                ui_type: ui_type.to_string(),
                message,
            })
        }
        "artifact-count" => {
            let count = data.parse::<u32>().unwrap_or(0);
            Ok(PackerEvent::ArtifactCount { count })
        }
        "artifact" => {
            let artifact_parts: Vec<&str> = data.splitn(6, ',').collect();
            if artifact_parts.len() >= 2 {
                Ok(PackerEvent::Artifact {
                    index: artifact_parts[0].parse().unwrap_or(0),
                    builder_type: artifact_parts[1].to_string(),
                    files: artifact_parts.get(5).unwrap_or(&"").to_string(),
                })
            } else {
                Err(Error::Parsing("Invalid artifact format".to_string()))
            }
        }
        _ => {
            // For unrecognized events, create a generic event
            Ok(PackerEvent::Generic {
                target: target.to_string(),
                event_type: event_type.to_string(),
                data,
            })
        }
    }
}

pub fn log_packer_event(event: &PackerEvent) {
    use console::style;

    match event {
        PackerEvent::UiMessage {
            ui_type, message, ..
        } => match ui_type.as_str() {
            "say" => println!("{}", message),
            "message" => println!("  {}", style(message).dim()),
            "error" => println!("{} {}", style("Error:").red().bold(), message),
            _ => println!("{}", message),
        },
        PackerEvent::ArtifactCount { count } => {
            println!(
                "{} Created {} artifact(s)",
                style("»").bold().blue(),
                style(count).bold()
            );
        }
        PackerEvent::Artifact {
            builder_type,
            files,
            ..
        } => {
            println!("  {} Artifact: {}", style(builder_type).bold(), files);
        }
        PackerEvent::Generic {
            event_type, data, ..
        } => {
            if !data.is_empty() {
                println!("  {}: {}", style(event_type).dim(), data);
            }
        }
    }
}

#[derive(Debug)]
pub enum PackerEvent {
    UiMessage {
        target: String,
        ui_type: String,
        message: String,
    },
    ArtifactCount {
        count: u32,
    },
    Artifact {
        index: u32,
        builder_type: String,
        files: String,
    },
    Generic {
        target: String,
        event_type: String,
        data: String,
    },
}

#[derive(Default)]
pub struct PackerBuildState {
    pub build_name: String,
    pub status: BuildStatus,
    pub errors: Vec<String>,
    pub artifacts: Vec<String>,
    pub build_duration: Option<std::time::Duration>,
}

#[derive(Debug, Default)]
pub enum BuildStatus {
    #[default]
    Starting,
    Running,
    Finished,
    Failed,
}

pub fn parse_template(content: &str) -> Result<Template> {
    let body: Body = hcl::from_str(content)?;

    let mut variables = HashMap::new();
    let mut sources = Vec::new();
    let mut provisioners = Vec::new();
    let mut dependencies = TemplateDependencies::default();
    let mut description = None;

    for block in body.blocks() {
        match block.identifier() {
            "variable" => {
                if let Some(var) = parse_variable(block)? {
                    if var.0 == "description" {
                        if let Some(default) = &var.1.default {
                            description = Some(default.clone());
                        }
                    }
                    variables.insert(var.0, var.1);
                }
            }
            "source" => {
                if let Some(source) = parse_source(block)? {
                    extract_source_dependencies(block, &mut dependencies)?;
                    sources.push(source);
                }
            }
            "build" => {
                extract_build_dependencies(block, &mut dependencies)?;
            }
            "provisioner" => {
                if let Some(provisioner) = parse_provisioner(block)? {
                    extract_provisioner_dependencies(block, &mut dependencies)?;
                    provisioners.push(provisioner);
                }
            }
            _ => {}
        }
    }

    Ok(Template::builder()
        .name(String::new())
        .content(content.to_string())
        .variables(variables)
        .sources(sources)
        .provisioners(provisioners)
        .dependencies(dependencies)
        .maybe_description(description)
        .build())
}

pub fn extract_description_from_body(body: &Body) -> Option<String> {
    for block in body.blocks() {
        if block.identifier() == "variable" {
            if let Some(var_name) = block.labels().first() {
                if var_name.as_str() == "description" {
                    for attr in block.body().attributes() {
                        if attr.key() == "default" {
                            return extract_string_from_expr(attr.expr());
                        }
                    }
                }
            }
        }
    }
    None
}

pub fn parse_variable(block: &Block) -> Result<Option<(String, Variable)>> {
    let var_name = block
        .labels()
        .first()
        .ok_or_else(|| Error::Template("Variable missing name".to_string()))?
        .as_str()
        .to_string();

    let mut var = Variable {
        var_type: VarType::String,
        default: None,
        description: None,
        required: true,
        enum_values: None,
        sensitive: false,
    };

    // Extract type
    if let Some(attr) = block.body().attributes().find(|a| a.key() == "type") {
        if let Some(type_str) = extract_string_from_expr(attr.expr()) {
            var.var_type = VarType::from(type_str.as_str());
        }
    }

    // Extract default value
    if let Some(attr) = block.body().attributes().find(|a| a.key() == "default") {
        var.default = extract_string_from_expr(attr.expr());
        var.required = false;
    }

    // Extract description
    if let Some(attr) = block.body().attributes().find(|a| a.key() == "description") {
        var.description = extract_string_from_expr(attr.expr());
    }

    // Extract sensitive flag
    if let Some(attr) = block.body().attributes().find(|a| a.key() == "sensitive") {
        var.sensitive = extract_bool_from_expr(attr.expr()).unwrap_or(false);
    }

    // Extract validation
    for nested_block in block.body().blocks() {
        if nested_block.identifier() == "validation" {
            if let Some(attr) = nested_block
                .body()
                .attributes()
                .find(|a| a.key() == "condition")
            {
                var.enum_values = extract_enum_validation_from_expr(attr.expr());
            }
        }
    }

    Ok(Some((var_name, var)))
}

pub fn parse_source(block: &Block) -> Result<Option<Source>> {
    let labels: Vec<_> = block.labels().iter().collect();
    if labels.len() < 2 {
        return Err(Error::Template("Invalid source block".to_string()));
    }

    let mut config = HashMap::new();
    for attr in block.body().attributes() {
        config.insert(attr.key().to_string(), format!("{:?}", attr.expr()));
    }

    Ok(Some(Source {
        source_type: labels[0].as_str().to_string(),
        name: labels[1].as_str().to_string(),
        config,
    }))
}

pub fn parse_provisioner(block: &Block) -> Result<Option<Provisioner>> {
    let prov_type = block
        .labels()
        .first()
        .ok_or_else(|| Error::Template("Provisioner missing type".to_string()))?
        .as_str()
        .to_string();

    let mut config = HashMap::new();
    for attr in block.body().attributes() {
        config.insert(attr.key().to_string(), format!("{:?}", attr.expr()));
    }

    Ok(Some(Provisioner {
        provisioner_type: prov_type,
        config,
    }))
}

pub fn extract_source_dependencies(block: &Block, deps: &mut TemplateDependencies) -> Result<()> {
    for attr in block.body().attributes() {
        match attr.key() {
            "http_directory" => {
                if let Some(dir) = extract_string_from_expr(attr.expr()) {
                    deps.http_directories.insert(dir);
                }
            }
            "floppy_files" => {
                if let Ok(files) = extract_string_array_from_expr(attr.expr()) {
                    for file in files {
                        if let Some(filename) = Path::new(&file).file_name() {
                            if let Some(name) = filename.to_str() {
                                deps.floppy_files.insert(name.to_string());
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }
    Ok(())
}

pub fn extract_build_dependencies(block: &Block, deps: &mut TemplateDependencies) -> Result<()> {
    for nested_block in block.body().blocks() {
        if nested_block.identifier() == "provisioner" {
            extract_provisioner_dependencies(nested_block, deps)?;
        }
    }
    Ok(())
}

pub fn extract_provisioner_dependencies(
    block: &Block,
    deps: &mut TemplateDependencies,
) -> Result<()> {
    if let Some(provisioner_type) = block.labels().first() {
        match provisioner_type.as_str() {
            "shell" | "powershell" => {
                for attr in block.body().attributes() {
                    match attr.key() {
                        "scripts" => {
                            if let Ok(scripts) = extract_string_array_from_expr(attr.expr()) {
                                for script in scripts {
                                    if let Some(filename) = Path::new(&script).file_name() {
                                        if let Some(name) = filename.to_str() {
                                            deps.script_files.insert(name.to_string());
                                        }
                                    }
                                }
                            }
                        }
                        "script" => {
                            if let Some(script) = extract_string_from_expr(attr.expr()) {
                                if let Some(filename) = Path::new(&script).file_name() {
                                    if let Some(name) = filename.to_str() {
                                        deps.script_files.insert(name.to_string());
                                    }
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
            "ansible" => {
                for attr in block.body().attributes() {
                    if attr.key() == "playbook_file" {
                        if let Some(playbook) = extract_string_from_expr(attr.expr()) {
                            if let Some(filename) = Path::new(&playbook).file_name() {
                                if let Some(name) = filename.to_str() {
                                    deps.provisioner_files.insert(name.to_string());
                                }
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }
    Ok(())
}

// Helper functions for extracting values from HCL expressions
fn extract_string_from_expr(expr: &Expression) -> Option<String> {
    match expr {
        Expression::String(s) => Some(s.clone()),
        Expression::Variable(v) => Some(v.to_string()),
        _ => None,
    }
}

fn extract_bool_from_expr(expr: &Expression) -> Option<bool> {
    match expr {
        Expression::Bool(b) => Some(*b),
        _ => None,
    }
}

fn extract_string_array_from_expr(expr: &Expression) -> Result<Vec<String>> {
    match expr {
        Expression::Array(arr) => {
            let mut strings = Vec::new();
            for item in arr {
                if let Some(s) = extract_string_from_expr(item) {
                    strings.push(s);
                }
            }
            Ok(strings)
        }
        _ => Err(Error::Template("Expected array expression".to_string())),
    }
}

fn extract_enum_validation_from_expr(_expr: &Expression) -> Option<Vec<String>> {
    // This would need more complex parsing of validation conditions
    // For now, return None as a placeholder
    None
}
