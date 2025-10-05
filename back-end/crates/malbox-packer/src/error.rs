use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("HCL parsing error: {0}")]
    Hcl(#[from] hcl::Error),
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("Template error: {0}")]
    Template(String),
    #[error("Build error: {0}")]
    Build(String),
    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),
    #[error("Packer execution failed: {0}")]
    PackerExecution(String),
    #[error("Template not found: {0}")]
    TemplateNotFound(String),
    #[error("Variable validation failed: {0}")]
    VariableValidation(String),
    #[error("Variable error: {0}")]
    Variable(String),
    #[error("Packer error: {0}")]
    Packer(String),
    #[error("Parsing error: {0}")]
    Parsing(String),
    #[error("IO Utils error: {0}")]
    IoUtils(#[from] malbox_io_utils::error::IoError),
}

pub type Result<T> = std::result::Result<T, Error>;
