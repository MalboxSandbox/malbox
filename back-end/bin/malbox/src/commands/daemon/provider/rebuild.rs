//! Rebuild daemon with current provider configuration.

use crate::commands::Command;
use crate::utils::provider_config::ProvidersEdit;
use clap::Parser;
use malbox_cli_common::context::Context;
use malbox_cli_common::error::{CliError, Result};
use malbox_installer::manifest::Manifest;

#[derive(Parser)]
#[command(about = "Rebuild daemon with current provider configuration")]
pub struct RebuildCommand {}

impl Command for RebuildCommand {
    async fn execute(self, _ctx: &Context) -> Result<()> {
        let edit = ProvidersEdit::load()?;
        let manifest = Manifest::load(&Manifest::default_path())
            .map_err(|e| CliError::CommandFailed(e.to_string()))?;
        let features = super::desired_features(&manifest, &edit.enabled())?;
        super::rebuild_daemon(&features).await
    }
}
