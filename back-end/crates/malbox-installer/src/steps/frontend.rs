use crate::config::FrontendSource;
use crate::error::{InstallError, Step};
use crate::github::GitHubClient;
use crate::progress::InstallProgress;
use std::path::{Path, PathBuf};

pub async fn execute(
    source: &FrontendSource,
    data_dir: &Path,
    github: &GitHubClient,
    release_tag: &str,
    progress: &dyn InstallProgress,
) -> crate::Result<PathBuf> {
    progress.started(Step::Frontend, "Installing front-end assets");

    let web_dir = data_dir.join("web");
    tokio::fs::create_dir_all(&web_dir).await?;

    match source {
        FrontendSource::Prebuilt { url } => {
            progress.progress(Step::Frontend, 10, "Downloading prebuilt front-end assets");
            let bytes = github.download_asset(url).await?;

            progress.progress(Step::Frontend, 80, "Extracting front-end assets");
            let decoder = flate2::read::GzDecoder::new(bytes.as_slice());
            let mut archive = tar::Archive::new(decoder);
            archive
                .unpack(&web_dir)
                .map_err(|e| InstallError::StepFailed {
                    step: Step::Frontend,
                    message: format!("failed to extract frontend assets: {e}"),
                })?;
        }
        FrontendSource::Compile => {
            progress.progress(Step::Frontend, 10, "Fetching source for front-end build");
            let source_url = format!(
                "https://github.com/malboxapp/malbox/archive/refs/tags/{}.tar.gz",
                release_tag
            );
            let bytes = github.download_asset(&source_url).await?;

            let tmp_dir = tempfile::tempdir().map_err(|e| InstallError::StepFailed {
                step: Step::Frontend,
                message: format!("failed to create temp dir: {e}"),
            })?;

            let decoder = flate2::read::GzDecoder::new(bytes.as_slice());
            let mut archive = tar::Archive::new(decoder);
            archive
                .unpack(tmp_dir.path())
                .map_err(|e| InstallError::StepFailed {
                    step: Step::Frontend,
                    message: format!("failed to extract source: {e}"),
                })?;

            let frontend_dir = tmp_dir.path().join("front-end");

            progress.progress(Step::Frontend, 20, "Installing npm dependencies");
            run_cmd("npm", &["install"], &frontend_dir, Step::Frontend).await?;

            progress.progress(Step::Frontend, 50, "Building front-end");
            run_cmd("npm", &["run", "build"], &frontend_dir, Step::Frontend).await?;

            progress.progress(Step::Frontend, 90, "Copying build output");
            let build_output = frontend_dir.join("build");
            copy_dir_recursive(&build_output, &web_dir).await?;
        }
    }

    progress.completed(Step::Frontend);
    Ok(web_dir)
}

async fn run_cmd(cmd: &str, args: &[&str], cwd: &Path, step: Step) -> crate::Result<()> {
    let status = tokio::process::Command::new(cmd)
        .args(args)
        .current_dir(cwd)
        .status()
        .await
        .map_err(|e| InstallError::StepFailed {
            step,
            message: format!("failed to run {cmd}: {e}"),
        })?;

    if !status.success() {
        return Err(InstallError::StepFailed {
            step,
            message: format!("{cmd} exited with non-zero status"),
        });
    }
    Ok(())
}

async fn copy_dir_recursive(src: &Path, dst: &Path) -> crate::Result<()> {
    tokio::fs::create_dir_all(dst).await?;
    let mut entries = tokio::fs::read_dir(src).await?;
    while let Some(entry) = entries.next_entry().await? {
        let dest_path = dst.join(entry.file_name());
        if entry.file_type().await?.is_dir() {
            Box::pin(copy_dir_recursive(&entry.path(), &dest_path)).await?;
        } else {
            tokio::fs::copy(entry.path(), dest_path).await?;
        }
    }
    Ok(())
}
