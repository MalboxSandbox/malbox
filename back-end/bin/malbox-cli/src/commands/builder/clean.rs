use crate::{commands::Command, error::Result, utils::progress::Progress};
use clap::Parser;
use malbox_config::Config;
use malbox_packer::build::BuildManager;

#[derive(Parser)]
pub struct CleanArgs {
    #[arg(short, long)]
    pub force: bool,
}

impl Command for CleanArgs {
    async fn execute(self, config: &Config) -> Result<()> {
        let builder = BuildManager::new(config.paths.clone());

        if !self.force {
            use dialoguer::{Confirm, theme::ColorfulTheme};
            let confirmed = Confirm::with_theme(&ColorfulTheme::default())
                .with_prompt("This will remove all cached build directories. Continue?")
                .default(false)
                .interact()
                .map_err(|e| crate::error::CliError::Dialoguer(e))?;

            if !confirmed {
                println!("Clean operation cancelled.");
                return Ok(());
            }
        }

        Progress::new()
            .run("Cleaning build cache...", async {
                builder
                    .clean_cache()
                    .await
                    .map_err(|e| crate::error::CliError::Packer(e))
            })
            .await?;

        println!("Build cache cleaned successfully!");

        Ok(())
    }
}
