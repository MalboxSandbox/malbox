//! Error type returned by CLI commands and the API client.

use thiserror::Error;

/// Any failure a CLI command can report to the user.
#[derive(Error, Debug)]
#[non_exhaustive]
pub enum CliError {
    /// Loading or validating the daemon configuration failed.
    #[error("Configuration error: {0}")]
    Config(#[from] malbox_config::ConfigError),
    /// A user-supplied argument was malformed or referred to nothing.
    #[error("Invalid argument: {0}")]
    InvalidArgument(String),
    /// A filesystem or terminal I/O operation failed.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    /// A command could not complete; the message explains why.
    #[error("Command failed: {0}")]
    CommandFailed(String),
    /// JSON encoding or decoding failed.
    #[error("JSON (de)serialization error: {0}")]
    Json(#[from] serde_json::Error),
    /// YAML encoding or decoding failed.
    #[error("YAML (de)serialization error: {0}")]
    Yaml(#[from] serde_yaml::Error),
    /// TOML input could not be parsed.
    #[error("TOML deserialization error: {0}")]
    TomlDe(#[from] toml::de::Error),
    /// A value could not be encoded as TOML.
    #[error("TOML serialization error: {0}")]
    TomlSer(#[from] toml::ser::Error),
    /// An interactive prompt failed.
    #[error("Dialoguer error: {0}")]
    Dialoguer(#[from] dialoguer::Error),
    /// The daemon API answered with a non-success status; holds status and message.
    #[error("API error: {0}")]
    Api(String),
    /// The HTTP request could not be sent or its body could not be read.
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),
}

/// Result alias using [`CliError`].
pub type Result<T> = std::result::Result<T, CliError>;
