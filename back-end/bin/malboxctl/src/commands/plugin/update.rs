use super::load_plugin_config;
use clap::Parser;
use malbox_cli_common::command::Command;
use malbox_cli_common::context::Context;
use malbox_cli_common::error::{CliError, Result};
use malbox_plugin_registry::client::RegistryClient;
use malbox_plugin_registry::lockfile::Lockfile;
use malbox_plugin_registry::resolve::Platform;
use malbox_plugin_registry::update::update_plugin;

#[derive(Parser)]
pub struct UpdateCommand {
    /// Plugin name to update (updates all if omitted)
    name: Option<String>,

    /// Re-install even if already at latest version
    #[arg(long)]
    force: bool,

    /// Show what would be updated without doing it
    #[arg(long)]
    dry_run: bool,
}

impl Command for UpdateCommand {
    async fn execute(self, _ctx: &Context) -> Result<()> {
        let (plugins_config, registry_config) = load_plugin_config().await?;
        let plugins_dir = &plugins_config.directory;
        let platform = Platform::current();

        let lockfile_path = Lockfile::lockfile_path(plugins_dir);
        let lockfile =
            Lockfile::load(&lockfile_path).map_err(|e| CliError::CommandFailed(e.to_string()))?;

        if lockfile.plugins.is_empty() {
            println!("  No tracked plugins to update.");
            return Ok(());
        }

        let client = RegistryClient::new(
            &registry_config.repository,
            registry_config.cache_dir.clone(),
        )
        .map_err(|e| CliError::CommandFailed(e.to_string()))?;

        println!("  \u{25b8} Checking for updates...\n");

        let plugin_names: Vec<String> = if let Some(ref name) = self.name {
            if !lockfile.plugins.contains_key(name) {
                return Err(CliError::CommandFailed(format!(
                    "plugin '{}' is not installed or not tracked",
                    name
                )));
            }
            vec![name.clone()]
        } else {
            lockfile.plugins.keys().cloned().collect()
        };

        let mut updated_count = 0usize;
        let mut up_to_date_count = 0usize;

        for name in &plugin_names {
            if self.dry_run {
                let locked = &lockfile.plugins[name];
                let (owner, repo) = locked.repository.split_once('/').ok_or_else(|| {
                    CliError::CommandFailed(format!("invalid repository: {}", locked.repository))
                })?;

                let gh = malbox_installer::github::GitHubClient::new(owner, repo)
                    .map_err(|e| CliError::CommandFailed(e.to_string()))?;
                let release = gh
                    .latest_release(malbox_installer::Channel::Nightly)
                    .await
                    .map_err(|e| CliError::CommandFailed(e.to_string()))?;
                let latest = release.version().to_string();

                if malbox_plugin_registry::update::needs_update(&locked.version, &latest) {
                    println!(
                        "  {:<20} {} \u{2192} {}   would update",
                        name, locked.version, latest
                    );
                    updated_count += 1;
                } else {
                    println!("  {:<20} {:<20}   up to date", name, locked.version);
                    up_to_date_count += 1;
                }
                continue;
            }

            match update_plugin(
                name,
                &lockfile,
                &client,
                plugins_dir,
                &platform,
                self.force,
                &mut |_, _| {},
            )
            .await
            {
                Ok(outcome) => {
                    if outcome.updated {
                        println!(
                            "  {:<20} {} \u{2192} {}   updated",
                            outcome.name, outcome.old_version, outcome.new_version
                        );
                        updated_count += 1;
                    } else {
                        println!(
                            "  {:<20} {:<20}   up to date",
                            outcome.name, outcome.old_version
                        );
                        up_to_date_count += 1;
                    }
                }
                Err(e) => {
                    println!("  {:<20} error: {}", name, e);
                }
            }
        }

        let untracked = lockfile
            .untracked_plugins(plugins_dir)
            .map_err(|e| CliError::CommandFailed(e.to_string()))?;
        if !untracked.is_empty() {
            println!(
                "\n  \u{26a0} {} untracked plugin(s) skipped: {}",
                untracked.len(),
                untracked.join(", ")
            );
            println!("    Re-install with malboxctl to enable updates.");
        }

        let verb = if self.dry_run {
            "would update"
        } else {
            "updated"
        };
        println!(
            "\n  \u{2713} {} plugin(s) {}, {} already up to date",
            updated_count, verb, up_to_date_count
        );

        Ok(())
    }
}
