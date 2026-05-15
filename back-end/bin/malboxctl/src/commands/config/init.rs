use crate::commands::Command;
use clap::Parser;
use malbox_cli_common::context::Context;
use malbox_cli_common::error::Result;
use malbox_config::{
    Config, Environment, LogLevel,
    core::{AnalysisConfig, DatabaseConfig, GeneralConfig, HttpConfig, PlatformAnalysisConfig},
    machinery::MachineryConfig,
    providers::ProvidersConfig,
    storage::PathConfig,
};
use std::collections::HashMap;
use std::io::{self, Write};
use std::path::PathBuf;

#[derive(Parser)]
pub struct InitArgs {
    /// Force overwrite existing configuration
    #[arg(short, long)]
    force: bool,
    /// Path to write the configuration file (defaults to ~/.config/malbox/malbox.toml)
    #[arg(short, long)]
    output: Option<PathBuf>,
}

impl Command for InitArgs {
    async fn execute(self, _ctx: &Context) -> Result<()> {
        // Determine output path
        let paths = PathConfig::new()?;
        let output_path = self
            .output
            .unwrap_or_else(|| paths.config_dir.join("malbox.toml"));

        // Check for existing configuration
        let existing_config = check_existing_config(&paths);

        if let Some(existing_path) = &existing_config
            && !self.force
        {
            println!("Warning: Configuration file already exists at:");
            println!("  {}", existing_path.display());
            println!();
            print!("Do you want to overwrite it? [y/N]: ");
            io::stdout().flush()?;

            let mut input = String::new();
            io::stdin().read_line(&mut input)?;
            let input = input.trim().to_lowercase();

            if input != "y" && input != "yes" {
                println!("Configuration generation cancelled.");
                return Ok(());
            }
        }

        // Generate default configuration
        let config = create_default_config(paths)?;

        // Ensure parent directory exists
        if let Some(parent) = output_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        // Serialize config to TOML
        let toml_str = toml::to_string_pretty(&config)?;

        // Write to file
        tokio::fs::write(&output_path, toml_str).await?;

        println!("Configuration file generated successfully at:");
        println!("  {}", output_path.display());
        println!();

        // Generate CLI config
        let cli_config_path = config.paths.config_dir.join("cli.toml");
        if !cli_config_path.exists() || self.force {
            let cli_config = malbox_config::CliConfig::default();
            let cli_toml_str = toml::to_string_pretty(&cli_config)?;
            tokio::fs::write(&cli_config_path, cli_toml_str).await?;
            println!("CLI configuration file generated at:");
            println!("  {}", cli_config_path.display());
        } else {
            println!("CLI configuration already exists at:");
            println!("  {}", cli_config_path.display());
            println!("  (use --force to overwrite)");
        }
        println!();

        Ok(())
    }
}

/// Check for existing configuration files in standard locations
fn check_existing_config(paths: &PathConfig) -> Option<PathBuf> {
    let user_config = paths.config_dir.join("malbox.toml");
    if user_config.exists() {
        return Some(user_config);
    }

    let system_config = PathBuf::from("/etc/malbox/malbox.toml");
    if system_config.exists() {
        return Some(system_config);
    }

    None
}

/// Create a default configuration with sensible defaults
fn create_default_config(paths: PathConfig) -> Result<Config> {
    let general = GeneralConfig {
        environment: Environment::Development,
        log_level: LogLevel::Info,
        debug: false,
        max_workers: 4,
        min_workers: 1,
        idle_timeout_ms: 60_000,
    };

    let http = HttpConfig {
        host: "127.0.0.1".to_string(),
        port: 8080,
        tls_enabled: false,
        cert_path: None,
        key_path: None,
        cors_origins: vec!["http://localhost:5173".to_string()],
        max_upload_size: 100 * 1024 * 1024, // 100 MB
    };

    let database = DatabaseConfig {
        host: "localhost".to_string(),
        port: 5432,
    };

    let providers = ProvidersConfig {
        enabled: vec![],
        default: None,
        configs: HashMap::new(),
    };

    let machinery = MachineryConfig::default();

    let analysis = AnalysisConfig {
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
    };

    let config = Config {
        paths,
        general,
        http,
        database,
        providers,
        machinery,
        images: None,
        guest_access: None,
        plugins: Default::default(),
        analysis,
        machines: Vec::new(),
    };

    Ok(config)
}
