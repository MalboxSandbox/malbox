use std::path::PathBuf;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("Configuration file not found")]
    NotFound,
    #[error("Failed to parse {file}: {error}")]
    Parse { file: String, error: String },
    #[error("Invalid value for {field}: {message}")]
    InvalidValue { field: String, message: String },
    #[error("Provider {0} not configured")]
    ProviderNotConfigured(String),
    #[error("Provider {0} not enabled")]
    ProviderNotEnabled(String),
    #[error("Invalid provider config for {provider}: {error}")]
    InvalidProviderConfig { provider: String, error: String },
    #[error("Path error: {message} for {path}")]
    PathError { message: String, path: PathBuf },
    #[error("Io error: {0}")]
    IoError(#[from] std::io::Error),
    #[error("TOML serialization error: {0}")]
    TomlSer(#[from] toml::ser::Error),
    #[error("TOML deserialization error: {0}")]
    TomlDe(#[from] toml::de::Error),
    #[error("Internal error: {0}")]
    Internal(String),
}

pub type Result<T> = std::result::Result<T, ConfigError>;
