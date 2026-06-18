use crate::error::{RegistryError, Result};
use crate::install::validate_extracted_plugin;
use malbox_installer::progress::ProgressObserver;
use std::path::{Path, PathBuf};
use std::process::ExitStatus;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, BufReader};

pub use malbox_installer::build_parse::{parse_cargo_artifact, parse_cmake_progress};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildSystem {
    Cargo,
    CMake,
    Python,
}

pub fn detect_build_system(repo_dir: &Path) -> Result<BuildSystem> {
    if repo_dir.join("Cargo.toml").exists() {
        Ok(BuildSystem::Cargo)
    } else if repo_dir.join("CMakeLists.txt").exists() {
        Ok(BuildSystem::CMake)
    } else if repo_dir.join("setup.py").exists() || repo_dir.join("pyproject.toml").exists() {
        Ok(BuildSystem::Python)
    } else {
        Err(RegistryError::BuildFailed {
            plugin: repo_dir.display().to_string(),
            reason: "no recognized build system found (expected Cargo.toml, CMakeLists.txt, setup.py, or pyproject.toml)".into(),
        })
    }
}

pub fn check_tool(name: &str) -> Result<PathBuf> {
    which::which(name).map_err(|_| RegistryError::ToolNotFound(name.to_string()))
}

fn validate_toolchain(build_system: BuildSystem) -> Result<()> {
    check_tool("git")?;
    match build_system {
        BuildSystem::Cargo => {
            check_tool("cargo")?;
        }
        BuildSystem::CMake => {
            check_tool("cmake")?;
        }
        BuildSystem::Python => {}
    }
    Ok(())
}

pub struct SourceBuildOutcome {
    pub plugin_dir: PathBuf,
    pub plugin_type: String,
}

pub async fn build_from_source(
    clone_url: &str,
    git_ref: &str,
    plugin_name: &str,
    plugins_dir: &Path,
    observer: &dyn ProgressObserver,
) -> Result<SourceBuildOutcome> {
    let clone_dir = tempfile::tempdir()?;

    observer.step_started("Cloning repository");
    run_command_streaming(
        "git",
        &["clone", "--depth", "1", "--branch", git_ref, clone_url, "."],
        clone_dir.path(),
        plugin_name,
        "git clone failed",
        observer,
    )
    .await
    .inspect_err(|e| observer.step_failed("Cloning repository", &e.to_string()))?;
    observer.step_completed("Cloning repository", "");

    observer.step_started("Detecting build system");
    let build_system = detect_build_system(clone_dir.path())
        .inspect_err(|e| observer.step_failed("Detecting build system", &e.to_string()))?;
    validate_toolchain(build_system)
        .inspect_err(|e| observer.step_failed("Detecting build system", &e.to_string()))?;
    observer.step_completed("Detecting build system", &format!("{build_system:?}"));

    match build_system {
        BuildSystem::Cargo => {
            observer.step_started("Building release binary");
            run_cargo_build(clone_dir.path(), plugin_name, observer)
                .await
                .inspect_err(|e| observer.step_failed("Building release binary", &e.to_string()))?;
            observer.step_completed("Building release binary", "");
        }
        BuildSystem::CMake => {
            observer.step_started("Building release binary");
            observer.step_started("Configuring build");
            run_command_streaming(
                "cmake",
                &["-B", "build", "-DCMAKE_BUILD_TYPE=Release"],
                clone_dir.path(),
                plugin_name,
                "cmake configure failed",
                observer,
            )
            .await
            .inspect_err(|e| observer.step_failed("Configuring build", &e.to_string()))?;
            observer.step_completed("Configuring build", "");
            observer.step_started("Building release binary");
            run_cmake_build(clone_dir.path(), plugin_name, observer)
                .await
                .inspect_err(|e| observer.step_failed("Building release binary", &e.to_string()))?;
            observer.step_completed("Building release binary", "");
        }
        BuildSystem::Python => {
            observer.step_started("Copying source files");
            observer.step_completed("Copying source files", "");
        }
    }

    std::fs::create_dir_all(plugins_dir)?;
    let staging = tempfile::tempdir_in(plugins_dir)?;
    let staging_plugin = staging.path().join(plugin_name);
    std::fs::create_dir_all(&staging_plugin)?;

    let manifest_src = clone_dir.path().join("plugin.toml");
    if !manifest_src.exists() {
        return Err(RegistryError::BuildFailed {
            plugin: plugin_name.to_string(),
            reason: "plugin.toml not found in repository root".into(),
        });
    }
    std::fs::copy(&manifest_src, staging_plugin.join("plugin.toml"))?;

    match build_system {
        BuildSystem::Cargo => {
            let binary = clone_dir.path().join("target/release").join(plugin_name);
            if !binary.exists() {
                return Err(RegistryError::BuildFailed {
                    plugin: plugin_name.to_string(),
                    reason: format!("binary '{}' not found in target/release/", plugin_name),
                });
            }
            std::fs::copy(&binary, staging_plugin.join(plugin_name))?;
            set_executable(&staging_plugin.join(plugin_name))?;
        }
        BuildSystem::CMake => {
            let binary = clone_dir.path().join("build").join(plugin_name);
            if !binary.exists() {
                return Err(RegistryError::BuildFailed {
                    plugin: plugin_name.to_string(),
                    reason: format!("binary '{}' not found in build/", plugin_name),
                });
            }
            std::fs::copy(&binary, staging_plugin.join(plugin_name))?;
            set_executable(&staging_plugin.join(plugin_name))?;
        }
        BuildSystem::Python => {
            copy_dir_contents(clone_dir.path(), &staging_plugin)?;
        }
    }

    observer.step_started("Validating plugin manifest");
    let manifest = validate_extracted_plugin(&staging_plugin)
        .inspect_err(|e| observer.step_failed("Validating plugin manifest", &e.to_string()))?;
    observer.step_completed("Validating plugin manifest", "");

    observer.step_started("Installing to plugins directory");
    let dest = plugins_dir.join(plugin_name);
    if dest.exists() {
        std::fs::remove_dir_all(&dest)?;
    }
    std::fs::rename(&staging_plugin, &dest)
        .inspect_err(|e| observer.step_failed("Installing to plugins directory", &e.to_string()))?;
    observer.step_completed("Installing to plugins directory", "");

    Ok(SourceBuildOutcome {
        plugin_type: format!("{:?}", manifest.plugin.plugin_type).to_lowercase(),
        plugin_dir: dest,
    })
}

// Task 2: Streaming command execution

async fn run_command_streaming(
    program: &str,
    args: &[&str],
    cwd: &Path,
    plugin_name: &str,
    context: &str,
    observer: &dyn ProgressObserver,
) -> Result<ExitStatus> {
    let mut child = tokio::process::Command::new(program)
        .args(args)
        .current_dir(cwd)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();

    let mut stdout_reader = BufReader::new(stdout).lines();
    let mut stderr_reader = BufReader::new(stderr).lines();

    let mut stderr_buf = String::new();

    loop {
        tokio::select! {
            line = stdout_reader.next_line() => {
                match line? {
                    Some(line) => observer.build_output(&line),
                    None => break,
                }
            }
            line = stderr_reader.next_line() => {
                match line? {
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

    while let Some(line) = stdout_reader.next_line().await? {
        observer.build_output(&line);
    }
    while let Some(line) = stderr_reader.next_line().await? {
        stderr_buf.push_str(&line);
        stderr_buf.push('\n');
        observer.build_output(&line);
    }

    let status = child.wait().await?;

    if !status.success() {
        return Err(RegistryError::BuildFailed {
            plugin: plugin_name.to_string(),
            reason: format!("{context}: {stderr_buf}"),
        });
    }
    Ok(status)
}

// Task 3: Cargo and CMake build runners

async fn run_cargo_build(
    cwd: &Path,
    plugin_name: &str,
    observer: &dyn ProgressObserver,
) -> Result<ExitStatus> {
    let total = count_cargo_packages(cwd).await.unwrap_or(0);

    let mut child = tokio::process::Command::new("cargo")
        .args(["build", "--release", "--message-format=json"])
        .current_dir(cwd)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();

    let mut stdout_reader = BufReader::new(stdout).lines();
    let mut stderr_reader = BufReader::new(stderr).lines();

    let mut compiled = 0usize;
    let mut stderr_buf = String::new();

    loop {
        tokio::select! {
            line = stdout_reader.next_line() => {
                match line? {
                    Some(line) => {
                        if let Some(_name) = parse_cargo_artifact(&line) {
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
                match line? {
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

    while let Some(line) = stdout_reader.next_line().await? {
        if let Some(_name) = parse_cargo_artifact(&line) {
            compiled += 1;
            let label = if total > 0 {
                format!("{compiled}/{total} crates")
            } else {
                format!("{compiled} crates")
            };
            observer.build_progress(compiled, total, &label);
        }
    }
    while let Some(line) = stderr_reader.next_line().await? {
        stderr_buf.push_str(&line);
        stderr_buf.push('\n');
        observer.build_output(&line);
    }

    let status = child.wait().await?;

    if !status.success() {
        return Err(RegistryError::BuildFailed {
            plugin: plugin_name.to_string(),
            reason: format!("cargo build failed: {stderr_buf}"),
        });
    }
    Ok(status)
}

async fn count_cargo_packages(cwd: &Path) -> Result<usize> {
    let output = tokio::process::Command::new("cargo")
        .args(["metadata", "--format-version=1"])
        .current_dir(cwd)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .await?;

    if !output.status.success() {
        return Ok(0);
    }

    let val: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    let count = val
        .get("packages")
        .and_then(|p| p.as_array())
        .map(|a| a.len())
        .unwrap_or(0);
    Ok(count)
}

async fn run_cmake_build(
    cwd: &Path,
    plugin_name: &str,
    observer: &dyn ProgressObserver,
) -> Result<ExitStatus> {
    let mut child = tokio::process::Command::new("cmake")
        .args(["--build", "build"])
        .current_dir(cwd)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();

    let mut stdout_reader = BufReader::new(stdout).lines();
    let mut stderr_reader = BufReader::new(stderr).lines();

    let mut stderr_buf = String::new();

    loop {
        tokio::select! {
            line = stdout_reader.next_line() => {
                match line? {
                    Some(line) => {
                        if let Some(pct) = parse_cmake_progress(&line) {
                            observer.build_progress(pct, 100, "");
                        }
                        observer.build_output(&line);
                    }
                    None => break,
                }
            }
            line = stderr_reader.next_line() => {
                match line? {
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

    while let Some(line) = stdout_reader.next_line().await? {
        if let Some(pct) = parse_cmake_progress(&line) {
            observer.build_progress(pct, 100, "");
        }
        observer.build_output(&line);
    }
    while let Some(line) = stderr_reader.next_line().await? {
        stderr_buf.push_str(&line);
        stderr_buf.push('\n');
        observer.build_output(&line);
    }

    let status = child.wait().await?;

    if !status.success() {
        return Err(RegistryError::BuildFailed {
            plugin: plugin_name.to_string(),
            reason: format!("cmake build failed: {stderr_buf}"),
        });
    }
    Ok(status)
}

fn set_executable(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let perms = std::fs::Permissions::from_mode(0o755);
    std::fs::set_permissions(path, perms)?;
    Ok(())
}

fn copy_dir_contents(src: &Path, dst: &Path) -> Result<()> {
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let target = dst.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if name_str == ".git" || name_str == "__pycache__" {
                continue;
            }
            std::fs::create_dir_all(&target)?;
            copy_dir_contents(&entry.path(), &target)?;
        } else {
            std::fs::copy(entry.path(), &target)?;
        }
    }
    Ok(())
}
