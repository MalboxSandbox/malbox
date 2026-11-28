use crate::error::{InstallError, Step};
use crate::github::{GitHubClient, Release};
use crate::progress::ProgressObserver;

/// Writes the default daemon configuration.
pub mod config;
/// Installs the daemon and CLI binaries, prebuilt or from source.
pub mod daemon;
/// Installs the front-end bundle, prebuilt or from source.
pub mod frontend;
/// Connects to or provisions the `PostgreSQL` database.
pub mod postgres;
/// Installs and enables systemd user services.
pub mod systemd;

pub(crate) async fn ensure_tool(tool: &str, hint: &str, step: Step) -> crate::Result<()> {
    let found = tokio::process::Command::new(tool)
        .arg("--version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .await
        .is_ok_and(|status| status.success());

    if found {
        Ok(())
    } else {
        Err(InstallError::StepFailed {
            step,
            message: format!("`{tool}` not found - {hint}"),
        })
    }
}

pub(crate) async fn download_with_progress(
    github: &GitHubClient,
    release: &Release,
    url: &str,
    label: &str,
    observer: &dyn ProgressObserver,
) -> crate::Result<Vec<u8>> {
    github
        .download_verified(release, url, &mut |done, total| {
            observer.download_progress(done, total, label);
        })
        .await
}
