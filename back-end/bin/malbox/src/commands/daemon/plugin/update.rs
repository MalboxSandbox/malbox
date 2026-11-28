use super::{confirm_reinstall, load_plugin_config};
use clap::Parser;
use malbox_cli_common::command::Command;
use malbox_cli_common::context::Context;
use malbox_cli_common::error::{CliError, Result};
use malbox_plugin_registry::client::RegistryClient;
use malbox_plugin_registry::lockfile::{Lockfile, PinKind};
use malbox_plugin_registry::resolve::Platform;
use malbox_plugin_registry::update::{UpdateMode, update_plugin};

#[derive(Parser)]
pub(crate) struct UpdateCommand {
    /// Plugin name to update (updates all if omitted)
    name: Option<String>,

    /// Re-install even if already at latest version
    #[arg(long)]
    force: bool,
}

impl Command for UpdateCommand {
    async fn execute(self, ctx: &Context) -> Result<()> {
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

        let plugin_names = self.target_names(&lockfile)?;

        let is_single = self.name.is_some();
        let mut updated_count = 0usize;
        let mut up_to_date_count = 0usize;

        for name in &plugin_names {
            if matches!(lockfile.plugins[name].pin, PinKind::Local) {
                println!(
                    "  {name:<20} local - skipped (reinstall with `malbox daemon plugin install <path>`)"
                );
                continue;
            }
            let mode = if self.force {
                UpdateMode::Force
            } else {
                UpdateMode::IfNewer
            };
            match update_plugin(name, &lockfile, &client, plugins_dir, &platform, mode).await {
                Ok(outcome) if outcome.updated => {
                    println!(
                        "  {:<20} {} \u{2192} {}   updated",
                        outcome.name, outcome.old_version, outcome.new_version
                    );
                    updated_count += 1;
                }
                Ok(outcome) => {
                    // Up to date. For a single-target run, offer to reinstall.
                    let locked = &lockfile.plugins[name];
                    let prompt = if matches!(locked.pin, PinKind::Commit) {
                        format!(
                            "{} is pinned to {}. Rebuild the same commit? ",
                            name,
                            locked.pin.label(&locked.version)
                        )
                    } else {
                        format!(
                            "{} is up to date ({}). Reinstall? ",
                            name, outcome.old_version
                        )
                    };

                    if is_single && (self.force || ctx.yes || confirm_reinstall(&prompt)) {
                        match update_plugin(
                            name,
                            &lockfile,
                            &client,
                            plugins_dir,
                            &platform,
                            UpdateMode::Force,
                        )
                        .await
                        {
                            Ok(o2) => {
                                println!(
                                    "  {:<20} {} \u{2192} {}   reinstalled",
                                    o2.name, o2.old_version, o2.new_version
                                );
                                updated_count += 1;
                            }
                            Err(e) => println!("  {name:<20} error: {e}"),
                        }
                    } else {
                        println!("  {:<20} {:<20}   up to date", name, outcome.old_version);
                        up_to_date_count += 1;
                    }
                }
                Err(e) => println!("  {name:<20} error: {e}"),
            }
        }

        report_untracked(&lockfile, plugins_dir)?;

        println!(
            "\n  \u{2713} {updated_count} plugin(s) updated, {up_to_date_count} already up to date"
        );

        Ok(())
    }
}

impl UpdateCommand {
    /// Plugins this run should update: the named one (which must be tracked)
    /// or every plugin in the lockfile.
    fn target_names(&self, lockfile: &Lockfile) -> Result<Vec<String>> {
        if let Some(name) = &self.name {
            if !lockfile.plugins.contains_key(name) {
                return Err(CliError::CommandFailed(format!(
                    "plugin '{name}' is not installed or not tracked"
                )));
            }
            Ok(vec![name.clone()])
        } else {
            Ok(lockfile.plugins.keys().cloned().collect())
        }
    }
}

/// Lists plugins present on disk but absent from the lockfile, which updates skip.
fn report_untracked(lockfile: &Lockfile, plugins_dir: &std::path::Path) -> Result<()> {
    let untracked = lockfile
        .untracked_plugins(plugins_dir)
        .map_err(|e| CliError::CommandFailed(e.to_string()))?;
    if !untracked.is_empty() {
        println!(
            "\n  \u{26a0} {} untracked plugin(s) skipped: {}",
            untracked.len(),
            untracked.join(", ")
        );
        println!("    Re-install with `malbox daemon plugin install` to enable updates.");
    }
    Ok(())
}
