use crate::{
    Environment, LogLevel, PathConfig, guest_access::GuestAccessConfig, images::ImagesConfig,
    machinery::MachineryConfig, plugins::PluginsConfig, providers::ProvidersConfig,
    provisioning::ProvisioningConfig,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub paths: PathConfig,
    pub general: GeneralConfig,
    pub http: HttpConfig,
    pub database: DatabaseConfig,
    #[serde(default)]
    pub providers: ProvidersConfig,
    pub machinery: MachineryConfig,
    #[serde(default)]
    pub images: Option<ImagesConfig>,
    #[serde(default)]
    pub provisioning: Option<ProvisioningConfig>,
    #[serde(default)]
    pub guest_access: Option<GuestAccessConfig>,
    #[serde(default)]
    pub plugins: PluginsConfig,
    pub analysis: AnalysisConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneralConfig {
    pub environment: Environment,
    #[serde(default = "default_log_level")]
    pub log_level: LogLevel,
    #[serde(default)]
    pub debug: bool,
    #[serde(default = "default_worker_threads")]
    pub worker_threads: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpConfig {
    pub host: String,
    pub port: u16,
    #[serde(default)]
    pub tls_enabled: bool,
    pub cert_path: Option<String>,
    pub key_path: Option<String>,
    #[serde(default)]
    pub cors_origins: Vec<String>,
    #[serde(default)]
    pub max_upload_size: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseConfig {
    pub host: String,
    pub port: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisConfig {
    pub timeout: u32,
    pub max_vms: u32,
    pub default_profile: String,
    pub windows: PlatformAnalysisConfig,
    pub linux: PlatformAnalysisConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlatformAnalysisConfig {
    pub default_profile: String,
    pub timeout: Option<u32>,
    pub max_vms: Option<u32>,
}

fn default_log_level() -> LogLevel {
    LogLevel::Info
}

fn default_worker_threads() -> usize {
    4
}
