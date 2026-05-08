use crate::config::DaemonSource;
use crate::error::{InstallError, Step};
use crate::github::GitHubClient;
use crate::progress::InstallProgress;
use std::path::{Path, PathBuf};

pub async fn execute(
    source: &DaemonSource,
    install_dir: &Path,
    github: &GitHubClient,
    release_tag: &str,
    progress: &dyn InstallProgress,
) -> crate::Result<PathBuf> {
    progress.started(Step::Daemon, "Installing daemon binary");

    let daemon_path = install_dir.join("malboxd");

    match source {
        DaemonSource::Prebuilt { url } => {
            progress.progress(Step::Daemon, 10, "Downloading prebuilt daemon binary");
            let bytes = github.download_asset(url).await?;

            progress.progress(Step::Daemon, 80, "Extracting daemon binary");
            extract_tarball(&bytes, &daemon_path)?;

            set_executable(&daemon_path)?;
        }
        DaemonSource::Compile { features } => {
            progress.progress(Step::Daemon, 10, "Fetching source tarball");
            let source_url = format!(
                "https://github.com/malboxapp/malbox/archive/refs/tags/{}.tar.gz",
                release_tag
            );
            let bytes = github.download_asset(&source_url).await?;

            let tmp_dir = tempfile::tempdir().map_err(|e| InstallError::StepFailed {
                step: Step::Daemon,
                message: format!("failed to create temp dir: {e}"),
            })?;

            progress.progress(Step::Daemon, 20, "Extracting source");
            extract_tarball(&bytes, tmp_dir.path())?;

            progress.progress(Step::Daemon, 30, "Compiling daemon with selected features");
            let feature_list = features.join(",");
            let status = tokio::process::Command::new("cargo")
                .arg("build")
                .arg("--release")
                .arg("--manifest-path")
                .arg(tmp_dir.path().join("back-end/Cargo.toml"))
                .arg("-p")
                .arg("malbox-daemon")
                .arg("--no-default-features")
                .arg("--features")
                .arg(&feature_list)
                .status()
                .await
                .map_err(|e| InstallError::StepFailed {
                    step: Step::Daemon,
                    message: format!("failed to run cargo build: {e}"),
                })?;

            if !status.success() {
                return Err(InstallError::StepFailed {
                    step: Step::Daemon,
                    message: "cargo build failed".to_string(),
                });
            }

            let built_binary = tmp_dir.path().join("back-end/target/release/malbox-daemon");
            tokio::fs::copy(&built_binary, &daemon_path).await?;
            set_executable(&daemon_path)?;
        }
    }

    progress.completed(Step::Daemon);
    Ok(daemon_path)
}

fn extract_tarball(bytes: &[u8], dest: &Path) -> crate::Result<()> {
    let decoder = flate2::read::GzDecoder::new(bytes);
    let mut archive = tar::Archive::new(decoder);
    archive.unpack(dest).map_err(|e| InstallError::StepFailed {
        step: Step::Daemon,
        message: format!("failed to extract tarball: {e}"),
    })
}

fn set_executable(path: &Path) -> crate::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let mut perms = std::fs::metadata(path)?.permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(path, perms)?;
    Ok(())
}
