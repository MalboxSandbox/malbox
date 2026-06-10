use crate::commands::Command;
use clap::Parser;
use malbox_cli_common::context::Context;
use malbox_cli_common::error::{CliError, Result};

/// Start the malbox daemon in the foreground.
///
/// Convenience wrapper that launches the malboxd binary.
/// For production, start via systemd: systemctl --user start malbox
#[derive(Parser)]
pub struct StartArgs {
    /// Log level passed to malboxd (error | warn | info | debug | trace)
    #[arg(long, default_value = "info")]
    pub log_level: String,
}

impl Command for StartArgs {
    async fn execute(self, _ctx: &Context) -> Result<()> {
        let malboxd = find_malboxd()?;
        let err = exec(&malboxd, &["--log-level", &self.log_level]);
        Err(CliError::CommandFailed(format!(
            "failed to exec malboxd at {}: {}",
            malboxd.display(),
            err
        )))
    }
}

fn find_malboxd() -> Result<std::path::PathBuf> {
    if let Ok(manifest) = malbox_installer::manifest::Manifest::load(
        &malbox_installer::manifest::Manifest::default_path(),
    ) && manifest.daemon.path.exists()
    {
        return Ok(manifest.daemon.path);
    }
    which::which("malboxd").map_err(|_| {
        CliError::CommandFailed("malboxd not found - run `malbox daemon install` first".to_string())
    })
}

#[cfg(unix)]
fn exec(path: &std::path::Path, args: &[&str]) -> std::io::Error {
    use std::os::unix::process::CommandExt;
    std::process::Command::new(path).args(args).exec()
}

#[cfg(not(unix))]
fn exec(path: &std::path::Path, args: &[&str]) -> std::io::Error {
    match std::process::Command::new(path).args(args).status() {
        Ok(status) => std::io::Error::other(format!("malboxd exited with {}", status)),
        Err(e) => e,
    }
}
