use crate::error::{RegistryError, Result};
use crate::install::validate_extracted_plugin;
use std::path::{Path, PathBuf};
use std::process::ExitStatus;

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
) -> Result<SourceBuildOutcome> {
    let clone_dir = tempfile::tempdir()?;

    run_command(
        "git",
        &["clone", "--depth", "1", "--branch", git_ref, clone_url, "."],
        clone_dir.path(),
        plugin_name,
        "git clone failed",
    )
    .await?;

    let build_system = detect_build_system(clone_dir.path())?;
    validate_toolchain(build_system)?;

    match build_system {
        BuildSystem::Cargo => {
            run_command(
                "cargo",
                &["build", "--release"],
                clone_dir.path(),
                plugin_name,
                "cargo build failed",
            )
            .await?;
        }
        BuildSystem::CMake => {
            run_command(
                "cmake",
                &["-B", "build", "-DCMAKE_BUILD_TYPE=Release"],
                clone_dir.path(),
                plugin_name,
                "cmake configure failed",
            )
            .await?;
            run_command(
                "cmake",
                &["--build", "build"],
                clone_dir.path(),
                plugin_name,
                "cmake build failed",
            )
            .await?;
        }
        BuildSystem::Python => {}
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

    let manifest = validate_extracted_plugin(&staging_plugin)?;

    let dest = plugins_dir.join(plugin_name);
    if dest.exists() {
        std::fs::remove_dir_all(&dest)?;
    }
    std::fs::rename(&staging_plugin, &dest)?;

    Ok(SourceBuildOutcome {
        plugin_type: format!("{:?}", manifest.plugin.plugin_type).to_lowercase(),
        plugin_dir: dest,
    })
}

async fn run_command(
    program: &str,
    args: &[&str],
    cwd: &Path,
    plugin_name: &str,
    context: &str,
) -> Result<ExitStatus> {
    let output = tokio::process::Command::new(program)
        .args(args)
        .current_dir(cwd)
        .output()
        .await?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(RegistryError::BuildFailed {
            plugin: plugin_name.to_string(),
            reason: format!("{context}: {stderr}"),
        });
    }
    Ok(output.status)
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
