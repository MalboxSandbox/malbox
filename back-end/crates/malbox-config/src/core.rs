use crate::{
    Environment, LogLevel, PathConfig, guest_access::GuestAccessConfig, images::ImagesConfig,
    machinery::MachineryConfig, machines::MachineConfig, orchestration::OrchestrationConfig,
    plugins::PluginsConfig, providers::ProvidersConfig,
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
    #[serde(default)]
    pub orchestration: OrchestrationConfig,
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
            orchestration: OrchestrationConfig::default(),
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

fn default_log_level() -> LogLevel {
    LogLevel::Info
}

fn default_db_host() -> String {
    "localhost".to_string()
}

fn default_db_port() -> u16 {
    5432
}

#[cfg(test)]
mod tests {
    use super::*;

    const BASE: &str = r#"
[general]
environment = "development"
[http]
host = "127.0.0.1"
port = 8080
[database]
"#;

    #[test]
    fn parses_without_orchestration_section() {
        let cfg: Config = toml::from_str(BASE).expect("parse");
        assert_eq!(
            cfg.orchestration.max_depth,
            OrchestrationConfig::default().max_depth
        );
    }

    #[test]
    fn rejects_legacy_analysis_section() {
        let src = format!("{BASE}\n[analysis]\ntimeout = 300\n");
        let err = toml::from_str::<Config>(&src).unwrap_err().to_string();
        assert!(err.contains("unknown field `analysis`"), "{err}");
    }

    #[test]
    fn rejects_legacy_worker_fields() {
        let src = BASE.replace(
            "environment = \"development\"",
            "environment = \"development\"\nmax_workers = 4",
        );
        let err = toml::from_str::<Config>(&src).unwrap_err().to_string();
        assert!(err.contains("unknown field `max_workers`"), "{err}");
    }
}
