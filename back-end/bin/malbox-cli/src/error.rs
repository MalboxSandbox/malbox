use thiserror::Error;

#[derive(Error, Debug)]
pub enum CliError {
    #[error("Configuration error: {0}")]
    Config(#[from] malbox_config::ConfigError),
    #[error("Invalid argument: {0}")]
    InvalidArgument(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Command failed: {0}")]
    CommandFailed(String),
    #[error("JSON (de)serialization error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("YAML (de)serialization error: {0}")]
    Yaml(#[from] serde_yaml::Error),
    #[error("TOML deserialization error: {0}")]
    TomlDe(#[from] toml::de::Error),
    #[error("TOML serialization error: {0}")]
    TomlSer(#[from] toml::ser::Error),
    #[error("Dialoguer error: {0}")]
    Dialoguer(#[from] dialoguer::Error),
    #[error("API error: {0}")]
    Api(String),
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),
}

pub type Result<T> = std::result::Result<T, CliError>;
