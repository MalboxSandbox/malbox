use std::path::PathBuf;
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Step {
    Daemon,
    Frontend,
    Postgres,
    Config,
    Systemd,
}

impl Step {
    /// Install steps in execution order.
    pub const ORDER: [Step; 5] = [
        Step::Daemon,
        Step::Frontend,
        Step::Postgres,
        Step::Config,
        Step::Systemd,
    ];

    /// True when `self` runs after `other` during installation. Drives
    /// resume: steps up to and including the last completed one are skipped.
    pub fn is_after(self, other: Step) -> bool {
        fn pos(step: Step) -> usize {
            Step::ORDER
                .iter()
                .position(|s| *s == step)
                .unwrap_or(usize::MAX)
        }
        pos(self) > pos(other)
    }
}

impl std::fmt::Display for Step {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
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

    #[error("manifest not found at {0} - run `malbox daemon install` first")]
    ManifestNotFound(PathBuf),

    #[error("already up to date (version {0})")]
    AlreadyUpToDate(String),

    #[error("github API error: {0}")]
    GitHub(String),

    #[error("checksum mismatch for {asset} - the download may be corrupted or tampered with")]
    ChecksumMismatch { asset: String },

    #[error("environment error: {0}")]
    Environment(String),

    #[error("network error: {0}")]
    Network(#[from] reqwest::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, InstallError>;

/// Attach step context to any error, converting it into a `StepFailed`.
/// Keeps the step modules free of repetitive `map_err` blocks.
pub(crate) trait StepCtx<T> {
    fn step_ctx(self, step: Step, what: &str) -> Result<T>;
}

impl<T, E: std::fmt::Display> StepCtx<T> for std::result::Result<T, E> {
    fn step_ctx(self, step: Step, what: &str) -> Result<T> {
        self.map_err(|e| InstallError::StepFailed {
            step,
            message: format!("{what}: {e}"),
        })
    }
}
