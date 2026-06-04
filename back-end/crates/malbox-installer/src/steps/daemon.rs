use crate::config::DaemonSource;
use crate::error::{InstallError, Step, StepCtx};
use crate::github::{GitHubClient, Release};
use crate::progress::InstallProgress;
use std::path::{Path, PathBuf};

pub struct InstallResult {
    pub malboxctl: PathBuf,
    pub malbox: PathBuf,
}

pub async fn execute(
    source: &DaemonSource,
    features: &[String],
    install_dir: &Path,
    github: &GitHubClient,
    release: &Release,
    progress: &dyn InstallProgress,
) -> crate::Result<InstallResult> {
    progress.started(Step::Daemon, "Installing malbox binaries");

    let malboxctl_path = install_dir.join("malboxctl");
    let malbox_path = install_dir.join("malbox");

    match source {
        DaemonSource::Prebuilt { url } => {
            let bytes = crate::steps::download_with_progress(
                github,
                release,
                url,
                "malboxctl",
                Step::Daemon,
                10..45,
                progress,
            )
            .await?;
            crate::archive::extract_binary(&bytes, "malboxctl", &malboxctl_path, Step::Daemon)?;

            match release_arch().and_then(|arch| release.find_malbox_asset(arch)) {
                Some(cli_asset) => {
                    let cli_bytes = crate::steps::download_with_progress(
                        github,
                        release,
                        &cli_asset.browser_download_url,
                        "malbox CLI",
                        Step::Daemon,
                        50..90,
                        progress,
                    )
                    .await?;
                    crate::archive::extract_binary(
                        &cli_bytes,
                        "malbox",
                        &malbox_path,
                        Step::Daemon,
                    )?;
                }
                None => progress.progress(
                    Step::Daemon,
                    50,
                    "Release has no malbox CLI asset for this platform; skipping",
                ),
            }
        }
        DaemonSource::Compile => {
            // Preflight before downloading megabytes of source.
            crate::steps::ensure_tool("cargo", "install Rust via https://rustup.rs", Step::Daemon)
                .await?;
            let host = host_triple().await?;

            progress.progress(Step::Daemon, 5, "Fetching source tarball");
            let source_url = release.source_archive_url(github.owner(), github.repo());
            let bytes = github
                .download_asset(&source_url, &mut |done, _| {
                    progress.progress(
                        Step::Daemon,
                        10,
                        &format!("Fetching source tarball ({:.1} MB)", done as f64 / 1e6),
                    );
                })
                .await?;

            let tmp_dir =
                tempfile::tempdir().step_ctx(Step::Daemon, "failed to create temp dir")?;

            progress.progress(Step::Daemon, 20, "Extracting source");
            crate::archive::extract_tarball(&bytes, tmp_dir.path(), Step::Daemon)?;
            let source_dir = crate::archive::find_extracted_dir(tmp_dir.path(), Step::Daemon)?;
            let manifest_path = source_dir.join("back-end/Cargo.toml");
            // Explicit --target and --target-dir make the output location
            // deterministic: user-level cargo config (build.target,
            // CARGO_TARGET_DIR) would otherwise silently relocate it.
            let target_dir = tmp_dir.path().join("target");

            progress.progress(
                Step::Daemon,
                30,
                "Compiling malboxctl with selected features",
            );
            let feature_list = features.join(",");
            run_cargo_build(
                &manifest_path,
                &target_dir,
                &host,
                &[
                    "-p",
                    "malboxctl",
                    "--no-default-features",
                    "--features",
                    &feature_list,
                ],
            )
            .await?;

            progress.progress(Step::Daemon, 70, "Compiling malbox CLI");
            run_cargo_build(&manifest_path, &target_dir, &host, &["-p", "malbox"]).await?;

            let built = target_dir.join(&host).join("release");
            install_built(&built.join("malboxctl"), &malboxctl_path)?;
            install_built(&built.join("malbox"), &malbox_path)?;
        }
    }

    progress.completed(Step::Daemon);
    Ok(InstallResult {
        malboxctl: malboxctl_path,
        malbox: malbox_path,
    })
}

async fn run_cargo_build(
    manifest_path: &Path,
    target_dir: &Path,
    target: &str,
    args: &[&str],
) -> crate::Result<()> {
    let status = tokio::process::Command::new("cargo")
        .args(["build", "--release", "--manifest-path"])
        .arg(manifest_path)
        .args(["--target", target, "--target-dir"])
        .arg(target_dir)
        .args(args)
        // The repo ships sqlx's prepared query cache; force offline mode so
        // a DATABASE_URL in the user's environment cannot break the build.
        .env("SQLX_OFFLINE", "true")
        .status()
        .await
        .step_ctx(Step::Daemon, "failed to run cargo build")?;

    if !status.success() {
        return Err(InstallError::StepFailed {
            step: Step::Daemon,
            message: format!("cargo build {} failed", args.join(" ")),
        });
    }
    Ok(())
}

fn install_built(src: &Path, dest: &Path) -> crate::Result<()> {
    let mut file = std::fs::File::open(src).step_ctx(
        Step::Daemon,
        &format!("failed to open built binary {}", src.display()),
    )?;
    crate::archive::write_executable(&mut file, dest, Step::Daemon)
}

/// Host target triple as reported by rustc. Pinned via `--target` so the
/// build output path is deterministic, and correct on musl hosts where a
/// fabricated `*-gnu` triple would be wrong.
async fn host_triple() -> crate::Result<String> {
    let output = tokio::process::Command::new("rustc")
        .arg("-vV")
        .output()
        .await
        .step_ctx(Step::Daemon, "failed to run rustc -vV")?;

    String::from_utf8_lossy(&output.stdout)
        .lines()
        .find_map(|line| line.strip_prefix("host: ").map(str::to_string))
        .ok_or_else(|| InstallError::StepFailed {
            step: Step::Daemon,
            message: "could not determine host target triple from rustc -vV".to_string(),
        })
}

/// Asset architecture label for prebuilt release artifacts, or `None` when
/// no prebuilt naming exists for this platform (callers fall back to
/// building from source).
pub fn release_arch() -> Option<&'static str> {
    match (std::env::consts::ARCH, std::env::consts::OS) {
        ("x86_64", "linux") => Some("linux-x64"),
        ("aarch64", "linux") => Some("linux-arm64"),
        _ => None,
    }
}
