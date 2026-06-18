use crate::config::FrontendSource;
use crate::error::{InstallError, Step, StepCtx};
use crate::github::{GitHubClient, Release};
use crate::progress::ProgressObserver;
use std::path::{Path, PathBuf};

pub struct FrontendResult {
    pub path: PathBuf,
    pub prev_path: Option<PathBuf>,
}

pub async fn execute(
    source: &FrontendSource,
    data_dir: &Path,
    github: &GitHubClient,
    release: &Release,
    observer: &dyn ProgressObserver,
) -> crate::Result<FrontendResult> {
    observer.step_started("Installing front-end assets");

    let web_dir = data_dir.join("web");
    let staging = data_dir.join("web.new");
    if staging.exists() {
        tokio::fs::remove_dir_all(&staging)
            .await
            .step_ctx(Step::Frontend, "failed to clear staging directory")?;
    }
    tokio::fs::create_dir_all(&staging).await?;

    match source {
        FrontendSource::Prebuilt { url } => {
            let bytes = crate::steps::download_with_progress(
                github,
                release,
                url,
                "front-end assets",
                observer,
            )
            .await?;

            observer.build_output("Extracting front-end assets");
            crate::archive::extract_tarball(&bytes, &staging, Step::Frontend)?;
        }
        FrontendSource::Compile => {
            crate::steps::ensure_tool(
                "pnpm",
                "install Node.js and pnpm to build the front-end from source",
                Step::Frontend,
            )
            .await?;

            observer.build_output("Fetching source for front-end build");
            let source_url = release.source_archive_url(github.owner(), github.repo());
            let bytes = github
                .download_asset(&source_url, &mut |done, total| {
                    observer.download_progress(done, total, "front-end source");
                })
                .await?;

            let tmp_dir =
                tempfile::tempdir().step_ctx(Step::Frontend, "failed to create temp dir")?;

            crate::archive::extract_tarball(&bytes, tmp_dir.path(), Step::Frontend)?;
            let source_dir = crate::archive::find_extracted_dir(tmp_dir.path(), Step::Frontend)?;
            let frontend_dir = source_dir.join("front-end");

            observer.build_output("Installing pnpm dependencies");
            run_cmd("pnpm", &["install"], &frontend_dir, Step::Frontend).await?;

            observer.build_output("Building front-end");
            run_cmd("pnpm", &["run", "build"], &frontend_dir, Step::Frontend).await?;

            observer.build_output("Copying build output");
            let build_output = frontend_dir.join("build");
            copy_dir_recursive(&build_output, &staging).await?;
        }
    }

    let prev = data_dir.join("web.prev");
    if prev.exists() {
        tokio::fs::remove_dir_all(&prev)
            .await
            .step_ctx(Step::Frontend, "failed to remove old backup bundle")?;
    }
    let had_previous = web_dir.exists();
    if had_previous {
        tokio::fs::rename(&web_dir, &prev)
            .await
            .step_ctx(Step::Frontend, "failed to back up current bundle")?;
    }
    tokio::fs::rename(&staging, &web_dir)
        .await
        .step_ctx(Step::Frontend, "failed to activate new bundle")?;

    observer.step_completed("Installing front-end assets", "");
    Ok(FrontendResult {
        path: web_dir,
        prev_path: had_previous.then_some(prev),
    })
}

async fn run_cmd(cmd: &str, args: &[&str], cwd: &Path, step: Step) -> crate::Result<()> {
    let status = tokio::process::Command::new(cmd)
        .args(args)
        .current_dir(cwd)
        .status()
        .await
        .step_ctx(step, &format!("failed to run {cmd}"))?;

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
