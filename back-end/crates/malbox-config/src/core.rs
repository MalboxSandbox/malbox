use crate::{
    Environment, LogLevel, PathConfig, guest_access::GuestAccessConfig, images::ImagesConfig,
    machinery::MachineryConfig, machines::MachineConfig, plugins::PluginsConfig,
    providers::ProvidersConfig,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    /// Filesystem layout. The loader always overwrites this with the
    /// XDG-derived paths, so config files may omit the section entirely.
    #[serde(default)]
    pub paths: PathConfig,
    pub general: GeneralConfig,
    pub http: HttpConfig,
    pub database: DatabaseConfig,
    #[serde(default)]
    pub providers: ProvidersConfig,
    #[serde(default)]
    pub machinery: MachineryConfig,
    #[serde(default)]
    pub images: Option<ImagesConfig>,
    #[serde(default)]
    pub guest_access: Option<GuestAccessConfig>,
    #[serde(default)]
    pub plugins: PluginsConfig,
    pub analysis: AnalysisConfig,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub machines: Vec<MachineConfig>,
}

impl Config {
    /// Stock defaults shared by `malbox daemon config init` and the installer.
    ///
    /// Callers tweak the returned value (environment, database, web dir,
    /// providers) instead of hand-writing TOML, so generated configuration
    /// can never drift from the schema the daemon parses.
    pub fn with_defaults(paths: PathConfig) -> Self {
        Self {
            paths,
            general: GeneralConfig {
                environment: Environment::Development,
                log_level: LogLevel::Info,
                debug: false,
                max_workers: default_max_workers(),
                min_workers: default_min_workers(),
                idle_timeout_ms: default_idle_timeout_ms(),
            },
            http: HttpConfig {
                host: "127.0.0.1".to_string(),
                port: 8080,
                tls_enabled: false,
                cert_path: None,
                key_path: None,
                cors_origins: vec!["http://localhost:5173".to_string()],
                max_upload_size: 100 * 1024 * 1024,
                web_dir: None,
            },
            database: DatabaseConfig::default(),
            providers: ProvidersConfig::default(),
            machinery: MachineryConfig::default(),
            images: None,
            guest_access: None,
            plugins: PluginsConfig::default(),
            analysis: AnalysisConfig {
                timeout: 300,
                max_vms: 5,
                default_profile: "default".to_string(),
                windows: PlatformAnalysisConfig {
                    default_profile: "win10_default".to_string(),
                    timeout: Some(300),
                    max_vms: Some(3),
                },
                linux: PlatformAnalysisConfig {
                    default_profile: "ubuntu_default".to_string(),
                    timeout: Some(300),
                    max_vms: Some(2),
                },
            },
            machines: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GeneralConfig {
    pub environment: Environment,
    #[serde(default = "default_log_level")]
    pub log_level: LogLevel,
    #[serde(default)]
    pub debug: bool,
    #[serde(default = "default_max_workers")]
    pub max_workers: usize,
    #[serde(default = "default_min_workers")]
    pub min_workers: usize,
    #[serde(default = "default_idle_timeout_ms")]
    pub idle_timeout_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
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
    /// Directory of the built front-end SPA to serve (same-origin) alongside
    /// the API. When unset the daemon runs API-only - e.g. in development,
    /// where Vite serves the UI and proxies `/v1` to the daemon.
    #[serde(default)]
    pub web_dir: Option<String>,
}

/// Name of the daemon's database. Not configurable: the daemon creates and
/// migrates it itself on first start, so there is nothing for an operator
/// to point elsewhere.
pub const DATABASE_NAME: &str = "malbox";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DatabaseConfig {
    /// PostgreSQL server hostname or IP address.
    #[serde(default = "default_db_host")]
    pub host: String,
    #[serde(default = "default_db_port")]
    pub port: u16,
    /// Role to connect as. When omitted, resolution matches psql: `PGUSER`
    /// from the environment, then the OS user the daemon runs as.
    pub user: Option<String>,
    /// Password for `user`. When omitted, `PGPASSWORD` still applies.
    /// Local trust/peer setups (the managed install, dev) need none.
    pub password: Option<String>,
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            host: default_db_host(),
            port: default_db_port(),
            user: None,
            password: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AnalysisConfig {
    pub timeout: u32,
    pub max_vms: u32,
    pub default_profile: String,
    pub windows: PlatformAnalysisConfig,
    pub linux: PlatformAnalysisConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlatformAnalysisConfig {
    pub default_profile: String,
    pub timeout: Option<u32>,
    pub max_vms: Option<u32>,
}

fn default_log_level() -> LogLevel {
    LogLevel::Info
}

fn default_db_host() -> String {
    "localhost".to_string()
}

fn default_db_port() -> u16 {
    5432
}

fn default_max_workers() -> usize {
    4
}

fn default_min_workers() -> usize {
    1
}

fn default_idle_timeout_ms() -> u64 {
    60_000
}
