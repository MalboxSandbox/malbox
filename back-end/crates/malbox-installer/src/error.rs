use std::path::PathBuf;
use thiserror::Error;

/// One stage of the install pipeline, in the order given by [`Step::ORDER`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[non_exhaustive]
pub enum Step {
    /// Install the daemon and CLI binaries.
    Daemon,
    /// Install the front-end bundle.
    Frontend,
    /// Connect to or set up `PostgreSQL`.
    Postgres,
    /// Generate the default daemon configuration.
    Config,
    /// Install systemd user services.
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
    #[must_use]
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

/// Failure of an install, upgrade, rebuild, or rollback operation.
#[derive(Error, Debug)]
#[non_exhaustive]
pub enum InstallError {
    /// An install step failed.
    #[error("{step} failed: {message}")]
    StepFailed {
        /// The step that failed.
        step: Step,
        /// What went wrong, including the underlying error.
        message: String,
    },

    /// The manifest exists but is malformed or inconsistent.
    #[error("manifest error: {0}")]
    Manifest(String),

    /// No manifest exists at the given path, so nothing is installed.
    #[error("manifest not found at {0} - run `malbox daemon install` first")]
    ManifestNotFound(PathBuf),

    /// The installed version already matches the latest release.
    #[error("already up to date (version {0})")]
    AlreadyUpToDate(String),

    /// The GitHub API returned an error or unexpected data.
    #[error("github API error: {0}")]
    GitHub(String),

    /// A downloaded asset did not match its published SHA-256 checksum.
    #[error("checksum mismatch for {asset} - the download may be corrupted or tampered with")]
    ChecksumMismatch {
        /// Name of the offending release asset.
        asset: String,
    },

    /// The host environment is unsuitable (missing tools, home directory, etc.).
    #[error("environment error: {0}")]
    Environment(String),

    /// HTTP request failure.
    #[error("network error: {0}")]
    Network(#[from] reqwest::Error),

    /// Filesystem or process I/O failure.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// JSON (de)serialization failure.
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}

/// Result alias whose error type is [`InstallError`].
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
