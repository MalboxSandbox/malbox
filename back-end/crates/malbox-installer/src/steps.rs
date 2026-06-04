use crate::error::{InstallError, Step};
use crate::github::{GitHubClient, Release};
use crate::progress::InstallProgress;

pub mod config;
pub mod daemon;
pub mod frontend;
pub mod postgres;
pub mod systemd;

/// Preflight check for an external tool, with an actionable hint. Run before
/// downloading megabytes of source only to fail at the build step.
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

/// Download a (checksum-verified) release asset, mapping byte progress onto
/// the `pct` slice of the step's progress bar.
pub(crate) async fn download_with_progress(
    github: &GitHubClient,
    release: &Release,
    url: &str,
    what: &str,
    step: Step,
    pct: std::ops::Range<u8>,
    progress: &dyn InstallProgress,
) -> crate::Result<Vec<u8>> {
    progress.progress(step, pct.start, &format!("Downloading {what}"));
    let span = u64::from(pct.end.saturating_sub(pct.start));
    github
        .download_verified(release, url, &mut |done, total| {
            if let Some(total) = total.filter(|t| *t > 0) {
                let done = done.min(total);
                let percent = pct.start + ((done * span) / total) as u8;
                progress.progress(
                    step,
                    percent,
                    &format!("Downloading {what} ({}%)", done * 100 / total),
                );
            }
        })
        .await
}
