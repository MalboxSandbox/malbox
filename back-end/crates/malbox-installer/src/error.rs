use std::path::PathBuf;
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Step {
    Nix,
    Daemon,
    Frontend,
    Postgres,
    Config,
    Systemd,
}

impl std::fmt::Display for Step {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Step::Nix => write!(f, "nix"),
            Step::Daemon => write!(f, "daemon"),
            Step::Frontend => write!(f, "frontend"),
            Step::Postgres => write!(f, "postgres"),
            Step::Config => write!(f, "config"),
            Step::Systemd => write!(f, "systemd"),
        }
    }
}

#[derive(Error, Debug)]
pub enum InstallError {
    #[error("{step} failed: {message}")]
    StepFailed { step: Step, message: String },

    #[error("manifest error: {0}")]
    Manifest(String),

    #[error("manifest not found at {0} - run `malbox install` first")]
    ManifestNotFound(PathBuf),

    #[error("already up to date (version {0})")]
    AlreadyUpToDate(String),

    #[error("github API error: {0}")]
    GitHub(String),

    #[error("network error: {0}")]
    Network(#[from] reqwest::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, InstallError>;
