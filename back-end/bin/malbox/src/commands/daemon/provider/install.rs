//! Install a provider.

use crate::commands::Command;
use crate::utils::provider_config::ProvidersEdit;
use clap::Parser;
use console::style;
use malbox_cli_common::context::Context;
use malbox_cli_common::error::{CliError, Result};
use malbox_cli_common::utils::format::Brand;
use malbox_installer::manifest::Manifest;

#[derive(Parser)]
#[command(
    about = "Install a provider (adds to config and rebuilds daemon)",
    after_help = "Examples:\n  malbox daemon provider install libvirt\n  malbox daemon provider install xen --no-rebuild"
)]
pub struct InstallCommand {
    /// Provider name (e.g., "libvirt", "xen")
    pub name: String,

    /// Skip automatic rebuild (only update config)
    #[arg(long)]
    pub no_rebuild: bool,
}

impl Command for InstallCommand {
    async fn execute(self, _ctx: &Context) -> Result<()> {
        super::ensure_known_provider(&self.name)?;

        let mut edit = ProvidersEdit::load()?;
        if edit.add(&self.name) {
            edit.save()?;
            println!(
                "  {} enabled '{}' in {}",
                Brand::success().apply_to("\u{2713}"),
                self.name,
                edit.path().display()
            );
        } else {
            println!("  '{}' is already enabled", self.name);
        }

        if self.no_rebuild {
            println!(
                "  Skipping rebuild - run {} to compile it into the daemon",
                style("malbox daemon provider rebuild").bold()
            );
            return Ok(());
        }

        let manifest = Manifest::load(&Manifest::default_path())
            .map_err(|e| CliError::CommandFailed(e.to_string()))?;
        let features = super::desired_features(&manifest, &edit.enabled())?;
        if super::binary_matches(&manifest, &features) {
            println!("  Daemon already carries this provider set - nothing to rebuild.");
            return Ok(());
        }

        super::rebuild_daemon(&features).await
    }
}
