use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstallConfig {
    pub nix: NixStrategy,
    pub providers: Vec<String>,
    pub daemon: DaemonSource,
    pub frontend: FrontendSource,
    pub postgres: PostgresStrategy,
    pub systemd: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NixStrategy {
    Install,
    Existing,
    Skip,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DaemonSource {
    Prebuilt { url: String },
    Compile { features: Vec<String> },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FrontendSource {
    Prebuilt { url: String },
    Compile,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PostgresStrategy {
    Existing { url: String },
    Setup,
}

#[derive(Debug, Clone)]
pub struct UpgradeConfig {
    pub force: bool,
    pub reconfigure: Option<InstallConfig>,
}
