use std::io;
use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum ManifestError {
    #[error("failed to read manifest file {path}: {source}", path = path.display())]
    Io {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("failed to parse manifest: {0}")]
    Parse(#[from] toml::de::Error),
    #[error("invalid manifest: {0}")]
    Invalid(String),
}
