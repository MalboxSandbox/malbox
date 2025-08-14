use crate::error::{Error, Result};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

mod manager;
pub mod vars;

pub use manager::TemplateManager;
pub use vars::Variable;

// IMPORTANT - We only support HCL syntax for packer templates, no JSON
// Since this is the new recommended way to describe templates, we prefer
// to leave JSON behind
// ---------------------
// Currently the structure for this is not the best, we should try to separate by logical concerns
// ex. the parsing logic would be in parser.rs, manager should probably be at top level, etc..

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Source {
    pub source_type: String,
    pub name: String,
    pub config: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Provisioner {
    pub provisioner_type: String,
    pub config: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TemplateDependencies {
    pub script_files: HashSet<String>,
    pub floppy_files: HashSet<String>,
    pub provisioner_files: HashSet<String>,
    pub http_directories: HashSet<String>,
}

impl TemplateDependencies {
    pub fn has_scripts(&self) -> bool {
        !self.script_files.is_empty()
    }
    pub fn has_floppy(&self) -> bool {
        !self.floppy_files.is_empty()
    }
    pub fn has_provisioners(&self) -> bool {
        !self.provisioner_files.is_empty()
    }
    pub fn has_http(&self) -> bool {
        !self.http_directories.is_empty()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Template {
    pub name: String,
    pub path: Option<PathBuf>,
    pub variables: HashMap<String, Variable>,
    pub sources: Vec<Source>,
    pub provisioners: Vec<Provisioner>,
    pub content: String,
    pub dependencies: TemplateDependencies,
    pub description: Option<String>,
}

impl Template {
    pub fn builder() -> TemplateBuilder {
        TemplateBuilder::default()
    }

    pub fn get_missing_variables(&self, provided: &HashMap<String, String>) -> Result<Vec<String>> {
        let mut missing = Vec::new();
        for (name, var) in &self.variables {
            if var.required && !provided.contains_key(name) {
                missing.push(name.clone());
            }
        }
        Ok(missing)
    }

    pub fn validate_all_variables(&self, variables: &HashMap<String, String>) -> Result<()> {
        let mut errors = Vec::new();

        for (name, var) in &self.variables {
            if let Some(value) = variables.get(name) {
                if let Err(e) = var.validate_and_format(value) {
                    errors.push(format!("{}: {}", name, e));
                }
            } else if var.required {
                errors.push(format!("{}: Missing required variable", name));
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(Error::Variable(errors.join("\n")))
        }
    }
}

#[derive(Default)]
pub struct TemplateBuilder {
    name: Option<String>,
    path: Option<PathBuf>,
    variables: Option<HashMap<String, Variable>>,
    sources: Option<Vec<Source>>,
    provisioners: Option<Vec<Provisioner>>,
    content: Option<String>,
    dependencies: Option<TemplateDependencies>,
    description: Option<String>,
}

impl TemplateBuilder {
    pub fn name(mut self, name: String) -> Self {
        self.name = Some(name);
        self
    }

    pub fn path(mut self, path: PathBuf) -> Self {
        self.path = Some(path);
        self
    }

    pub fn variables(mut self, variables: HashMap<String, Variable>) -> Self {
        self.variables = Some(variables);
        self
    }

    pub fn sources(mut self, sources: Vec<Source>) -> Self {
        self.sources = Some(sources);
        self
    }

    pub fn provisioners(mut self, provisioners: Vec<Provisioner>) -> Self {
        self.provisioners = Some(provisioners);
        self
    }

    pub fn content(mut self, content: String) -> Self {
        self.content = Some(content);
        self
    }

    pub fn dependencies(mut self, dependencies: TemplateDependencies) -> Self {
        self.dependencies = Some(dependencies);
        self
    }

    pub fn maybe_description(mut self, description: Option<String>) -> Self {
        self.description = description;
        self
    }

    pub fn build(self) -> Template {
        Template {
            name: self.name.unwrap_or_default(),
            path: self.path,
            variables: self.variables.unwrap_or_default(),
            sources: self.sources.unwrap_or_default(),
            provisioners: self.provisioners.unwrap_or_default(),
            content: self.content.unwrap_or_default(),
            dependencies: self.dependencies.unwrap_or_default(),
            description: self.description,
        }
    }
}
