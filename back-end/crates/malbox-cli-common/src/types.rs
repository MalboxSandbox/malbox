//! Value enums accepted as CLI arguments.

use clap::ValueEnum;
use malbox_config::Platform as InfraPlatformType;
use serde::{Deserialize, Serialize};

/// Guest operating system family selectable on the command line.
#[derive(Clone, ValueEnum, Debug, Serialize, Deserialize, PartialEq)]
#[non_exhaustive]
pub enum PlatformType {
    /// Microsoft Windows guests.
    Windows,
    /// Linux guests.
    Linux,
}

impl From<PlatformType> for InfraPlatformType {
    fn from(value: PlatformType) -> Self {
        match value {
            PlatformType::Linux => InfraPlatformType::Linux,
            PlatformType::Windows => InfraPlatformType::Windows,
        }
    }
}

/// How command results are rendered to stdout.
#[derive(Clone, ValueEnum, Debug, Serialize, Deserialize, Default)]
#[non_exhaustive]
pub enum OutputFormat {
    /// Human-readable tables and styled text.
    #[default]
    Text,
    /// Machine-readable JSON.
    Json,
}
