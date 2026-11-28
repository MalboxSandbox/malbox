use std::path::PathBuf;
use thiserror::Error;

/// Failure while locating, parsing, or validating malbox configuration.
#[derive(Error, Debug)]
#[non_exhaustive]
pub enum ConfigError {
    /// No configuration file was found in any of the searched locations.
    #[error("Configuration file not found")]
    NotFound,
    /// A configuration file could not be read or deserialized.
    #[error("Failed to parse {file}: {error}")]
    Parse {
        /// Path of the offending file.
        file: String,
        /// Underlying read or TOML error message.
        error: String,
    },
    /// A field holds a value outside its accepted set.
    #[error("Invalid value for {field}: {message}")]
    InvalidValue {
        /// Name of the field or type being parsed.
        field: String,
        /// Why the value was rejected.
        message: String,
    },
    /// A provider was requested that has no configuration section.
    #[error("Provider {0} not configured")]
    ProviderNotConfigured(String),
    /// A provider is configured but disabled.
    #[error("Provider {0} not enabled")]
    ProviderNotEnabled(String),
    /// A provider's configuration table failed to deserialize.
    #[error("Invalid provider config for {provider}: {error}")]
    InvalidProviderConfig {
        /// Provider name.
        provider: String,
        /// Deserialization error message.
        error: String,
    },
    /// A filesystem path could not be resolved or created.
    #[error("Path error: {message} for {path}")]
    PathError {
        /// What went wrong with the path.
        message: String,
        /// The path involved.
        path: PathBuf,
    },
    /// Generic I/O failure.
    #[error("Io error: {0}")]
    IoError(#[from] std::io::Error),
    /// Serializing configuration to TOML failed.
    #[error("TOML serialization error: {0}")]
    TomlSer(#[from] toml::ser::Error),
    /// Deserializing TOML into configuration failed.
    #[error("TOML deserialization error: {0}")]
    TomlDe(#[from] toml::de::Error),
    /// Unexpected internal failure.
    #[error("Internal error: {0}")]
    Internal(String),
}

/// Result alias whose error type is [`ConfigError`].
pub type Result<T> = std::result::Result<T, ConfigError>;
