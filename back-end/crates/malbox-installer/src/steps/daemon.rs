use crate::config::DaemonSource;
use crate::error::{InstallError, Step, StepCtx};
use crate::github::{GitHubClient, Release};
use crate::progress::ProgressObserver;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, BufReader};

pub struct InstallResult {
    pub daemon: PathBuf,
    pub malbox: PathBuf,
}

pub async fn execute(
    source: &DaemonSource,
    features: &[String],
    install_dir: &Path,
    github: &GitHubClient,
    release: &Release,
    observer: &dyn ProgressObserver,
) -> crate::Result<InstallResult> {
    observer.step_started("Installing malbox binaries");

    let daemon_path = install_dir.join("malboxd");
    let malbox_path = install_dir.join("malbox");

    match source {
        DaemonSource::Prebuilt { url } => {
            let bytes =
                crate::steps::download_with_progress(github, release, url, "malboxd", observer)
                    .await?;
            crate::archive::extract_binary(&bytes, "malboxd", &daemon_path, Step::Daemon)?;

            match release_arch().and_then(|arch| release.find_malbox_asset(arch)) {
                Some(cli_asset) => {
                    let cli_bytes = crate::steps::download_with_progress(
                        github,
                        release,
                        &cli_asset.browser_download_url,
                        "malbox CLI",
                        observer,
                    )
                    .await?;
                    crate::archive::extract_binary(
                        &cli_bytes,
                        "malbox",
                        &malbox_path,
                        Step::Daemon,
                    )?;
                }
                None => observer
                    .build_output("Release has no malbox CLI asset for this platform; skipping"),
            }
        }
        DaemonSource::Compile => {
            crate::steps::ensure_tool("cargo", "install Rust via https://rustup.rs", Step::Daemon)
                .await?;
            let host = host_triple().await?;

            observer.build_output("Fetching source tarball");
            let source_url = release.source_archive_url(github.owner(), github.repo());
            let bytes = github
                .download_asset(&source_url, &mut |done, total| {
                    observer.download_progress(done, total, "source tarball");
                })
                .await?;

            let tmp_dir =
                tempfile::tempdir().step_ctx(Step::Daemon, "failed to create temp dir")?;

            observer.build_output("Extracting source");
            crate::archive::extract_tarball(&bytes, tmp_dir.path(), Step::Daemon)?;
            let source_dir = crate::archive::find_extracted_dir(tmp_dir.path(), Step::Daemon)?;
            let manifest_path = source_dir.join("back-end/Cargo.toml");
            let target_dir = tmp_dir.path().join("target");

            let feature_list = features.join(",");
            run_cargo_build(
                &manifest_path,
                &target_dir,
                &host,
                &[
                    "-p",
                    "malboxd",
                    "--no-default-features",
                    "--features",
                    &feature_list,
                ],
                observer,
            )
            .await?;

            run_cargo_build(
                &manifest_path,
                &target_dir,
                &host,
                &["-p", "malbox"],
                observer,
            )
            .await?;

            let built = target_dir.join(&host).join("release");
            install_built(&built.join("malboxd"), &daemon_path)?;
            install_built(&built.join("malbox"), &malbox_path)?;
        }
    }

    observer.step_completed("Installing malbox binaries", "");
    Ok(InstallResult {
        daemon: daemon_path,
        malbox: malbox_path,
    })
}

async fn run_cargo_build(
    manifest_path: &Path,
    target_dir: &Path,
    target: &str,
    args: &[&str],
    observer: &dyn ProgressObserver,
) -> crate::Result<()> {
    let total = count_cargo_packages(manifest_path).await.unwrap_or(0);

    let mut child = tokio::process::Command::new("cargo")
        .args([
            "build",
            "--release",
            "--message-format=json",
            "--manifest-path",
        ])
        .arg(manifest_path)
        .args(["--target", target, "--target-dir"])
        .arg(target_dir)
        .args(args)
        .env("SQLX_OFFLINE", "true")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .step_ctx(Step::Daemon, "failed to run cargo build")?;

    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();

    let mut stdout_reader = BufReader::new(stdout).lines();
    let mut stderr_reader = BufReader::new(stderr).lines();

    let mut compiled = 0usize;
    let mut stderr_buf = String::new();

    loop {
        tokio::select! {
            line = stdout_reader.next_line() => {
                match line.step_ctx(Step::Daemon, "reading cargo stdout")? {
                    Some(line) => {
                        if crate::build_parse::parse_cargo_artifact(&line).is_some() {
                            compiled += 1;
                            let display = compiled.min(total);
                            let label = if total > 0 {
                                format!("{display}/{total} crates")
                            } else {
                                format!("{compiled} crates")
                            };
                            observer.build_progress(compiled, total, &label);
                        }
                    }
                    None => break,
                }
            }
            line = stderr_reader.next_line() => {
                match line.step_ctx(Step::Daemon, "reading cargo stderr")? {
                    Some(line) => {
                        stderr_buf.push_str(&line);
                        stderr_buf.push('\n');
                        observer.build_output(&line);
                    }
                    None => break,
                }
            }
        }
    }

    while let Some(line) = stdout_reader
        .next_line()
        .await
        .step_ctx(Step::Daemon, "reading cargo stdout")?
    {
        if crate::build_parse::parse_cargo_artifact(&line).is_some() {
            compiled += 1;
            let label = if total > 0 {
                format!("{compiled}/{total} crates")
            } else {
                format!("{compiled} crates")
            };
            observer.build_progress(compiled, total, &label);
        }
    }
    while let Some(line) = stderr_reader
        .next_line()
        .await
        .step_ctx(Step::Daemon, "reading cargo stderr")?
    {
        stderr_buf.push_str(&line);
        stderr_buf.push('\n');
        observer.build_output(&line);
    }

    let status = child
        .wait()
        .await
        .step_ctx(Step::Daemon, "waiting for cargo build")?;

    if !status.success() {
        return Err(InstallError::StepFailed {
            step: Step::Daemon,
            message: format!("cargo build {} failed", args.join(" ")),
        });
    }
    Ok(())
}

async fn count_cargo_packages(manifest_path: &Path) -> crate::Result<usize> {
    let output = tokio::process::Command::new("cargo")
        .args(["metadata", "--format-version=1", "--manifest-path"])
        .arg(manifest_path)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .await
        .step_ctx(Step::Daemon, "failed to run cargo metadata")?;

    if !output.status.success() {
        return Ok(0);
    }

    let val: serde_json::Value =
        serde_json::from_slice(&output.stdout).map_err(|e| InstallError::StepFailed {
            step: Step::Daemon,
            message: format!("failed to parse cargo metadata: {e}"),
        })?;
    let count = val
        .get("packages")
        .and_then(|p| p.as_array())
        .map(|a| a.len())
        .unwrap_or(0);
    Ok(count)
}

fn install_built(src: &Path, dest: &Path) -> crate::Result<()> {
    let mut file = std::fs::File::open(src).step_ctx(
        Step::Daemon,
        &format!("failed to open built binary {}", src.display()),
    )?;
    crate::archive::write_executable(&mut file, dest, Step::Daemon)
}

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

pub fn release_arch() -> Option<&'static str> {
    match (std::env::consts::ARCH, std::env::consts::OS) {
        ("x86_64", "linux") => Some("linux-x64"),
        ("aarch64", "linux") => Some("linux-arm64"),
        _ => None,
    }
}
