use crate::impl_display_fromstr;
use serde::{Deserialize, Serialize};

mod macros;

/// Guest operating system family of an analysis machine.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
#[expect(
    clippy::exhaustive_enums,
    reason = "matched exhaustively by malbox-database platform mapping"
)]
pub enum Platform {
    /// Microsoft Windows guests.
    Windows,
    /// Linux guests.
    Linux,
}

impl_display_fromstr!(Platform,
    Windows => "windows",
    Linux => "linux"
);

/// Deployment environment the daemon runs in.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
#[non_exhaustive]
pub enum Environment {
    /// Local development setup.
    Development,
    /// Pre-production staging deployment.
    Staging,
    /// Production deployment.
    Production,
}

impl_display_fromstr!(Environment,
    Development => "development",
    Staging => "staging",
    Production => "production"
);

/// Minimum severity of log records the daemon emits.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
#[non_exhaustive]
pub enum LogLevel {
    /// Only errors.
    Error,
    /// Warnings and above.
    Warn,
    /// Informational messages and above.
    Info,
    /// Debug messages and above.
    Debug,
    /// Everything, including trace output.
    Trace,
}

impl_display_fromstr!(LogLevel,
    Error => "error",
    Warn => "warn",
    Info => "info",
    Debug => "debug",
    Trace => "trace"
);
