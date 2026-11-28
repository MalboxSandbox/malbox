//! Error type for manifest loading and validation.

use std::io;
use std::path::PathBuf;

/// Failure while reading, parsing or validating a plugin manifest.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum ManifestError {
    #[error("failed to read manifest file {path}: {source}", path = path.display())]
    /// The manifest file could not be read.
    Io {
        /// Path of the manifest that failed to read.
        path: PathBuf,
        /// Underlying I/O error.
        #[source]
        source: io::Error,
    },
    #[error("failed to parse manifest: {0}")]
    /// The manifest is not valid TOML or does not match the schema.
    Parse(#[from] toml::de::Error),
    #[error("invalid manifest: {0}")]
    /// The manifest parsed but violates a semantic rule; message explains which.
    Invalid(String),
}
