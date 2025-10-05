use super::parser::{PackerBuildState, parse_packer_event};
use crate::error::{Error, Result};
use crate::parser::log_packer_event;
use crate::templates::{Template, TemplateManager};
use malbox_config::{PathConfig, Platform};
use malbox_io_utils::process::{AsyncCommand, OutputSource};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use tokio::fs;
use tracing::{debug, error, info, warn};

#[derive(Debug, Clone)]
pub struct BuildConfig {
    pub platform: Platform,
    pub name: String,
    pub template_path: PathBuf,
    pub iso: Option<String>,
    pub force: bool,
    pub working_dir: Option<PathBuf>,
    pub variables: HashMap<String, String>,
    pub only: Option<String>,
}

pub struct BuildManager {
    config: PathConfig,
}

async fn copy_directory(from: &Path, to: &Path) -> Result<()> {
    if !to.exists() {
        fs::create_dir_all(to).await?;
    }

    let mut entries = fs::read_dir(from).await?;
    while let Some(entry) = entries.next_entry().await? {
        let path = entry.path();
        if let Some(file_name) = path.file_name() {
            let target = to.join(file_name);

            if path.is_dir() {
                Box::pin(copy_directory(&path, &target)).await?;
            } else {
                if let Some(file_str) = file_name.to_str() {
                    if file_str.starts_with('.') || file_str.ends_with('~') {
                        continue;
                    }
                }

                if let Err(e) = fs::copy(&path, &target).await {
                    warn!("Failed to copy {:?} to {:?}: {}", path, target, e);
                } else {
                    debug!("Copied {:?} to {:?}", path, target);
                }
            }
        }
    }

    Ok(())
}

impl BuildManager {
    pub fn new(config: PathConfig) -> Self {
        Self { config }
    }

    /// Initialize packer plugins for templates in the configured directory.
    pub async fn initialize(&self) -> Result<()> {
        self.validate_packer_binary().await?;
        self.initialize_plugins().await?;
        Ok(())
    }

    /// Validate that packer binary is available and executable.
    pub async fn validate_packer_binary(&self) -> Result<()> {
        info!("Validating packer installation");

        let mut cmd = AsyncCommand::new("packer").args(&["version"]);

        let output = cmd.run().await?;

        if !output.success() {
            return Err(Error::PackerExecution(
                "Packer binary not found or not executable. Please install packer.".to_string(),
            ));
        }

        info!("Packer binary validated successfully");
        Ok(())
    }

    /// Initialize packer plugins for all templates.
    pub async fn initialize_plugins(&self) -> Result<()> {
        info!("Initializing packer plugins");

        // Initialize plugins for the packer directory and common templates
        let packer_dir = &self.config.packer_dir;
        if packer_dir.exists() {
            self.init_plugins_in_directory(packer_dir).await?;
        }

        // Initialize plugins for platform-specific templates
        for platform in ["windows", "linux"] {
            let platform_dir = packer_dir.join("templates").join(platform);
            if platform_dir.exists() {
                self.init_plugins_in_directory(&platform_dir).await?;
            }
        }

        info!("Packer plugins initialized successfully");
        Ok(())
    }

    /// Initialize plugins in a specific directory.
    async fn init_plugins_in_directory(&self, dir: &Path) -> Result<()> {
        debug!("Initializing plugins in directory: {:?}", dir);

        let mut cmd = AsyncCommand::new("packer")
            .args(&["init", "."])
            .current_dir(dir);

        let output = cmd.run().await?;

        if !output.success() {
            warn!("Failed to initialize plugins in directory: {:?}", dir);
            debug!("Packer init output: {}", output.stdout());
            debug!("Packer init stderr: {}", output.stderr());
        } else {
            debug!("Successfully initialized plugins in directory: {:?}", dir);
        }

        Ok(())
    }

    /// Clean all cached build directories.
    pub async fn clean_cache(&self) -> Result<()> {
        let cache_builds_dir = self.config.cache_dir.join("builds");

        if !cache_builds_dir.exists() {
            info!("No cache directory found, nothing to clean");
            return Ok(());
        }

        info!("Cleaning cache directory: {:?}", cache_builds_dir);

        let mut entries = fs::read_dir(&cache_builds_dir).await?;
        let mut removed_count = 0;

        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.is_dir() {
                if let Err(e) = fs::remove_dir_all(&path).await {
                    warn!("Failed to remove directory {:?}: {}", path, e);
                } else {
                    debug!("Removed build cache directory: {:?}", path);
                    removed_count += 1;
                }
            }
        }

        info!("Cleaned {} build cache directories", removed_count);
        Ok(())
    }

    pub async fn build(&self, config: BuildConfig) -> Result<()> {
        let build_dir = self.prepare_build_dir(&config).await?;
        debug!("Build dir prepared: {:#?}", build_dir);

        // Initialize plugins in the build directory if needed
        self.init_plugins_in_directory(&build_dir).await?;

        let template_file = self.find_template_file(&build_dir)?;
        debug!("Using template file: {:?}", template_file);

        let mut args = Vec::new();
        args.push("build");
        args.push("-timestamp-ui");
        args.push("-color=false");
        args.push("-machine-readable");

        if config.force {
            args.push("-force");
        }

        args.push("-on-error=cleanup");

        // Add --only flag if specified
        if let Some(only) = &config.only {
            args.push("-only");
            args.push(only);
        }

        let vars_file = build_dir.join("variables.auto.pkrvars.hcl");
        if vars_file.exists() {
            args.push("-var-file");
            args.push("variables.auto.pkrvars.hcl");
        }

        let filename = template_file.file_name().unwrap().to_str().unwrap();
        args.push(filename);

        let mut cmd = AsyncCommand::new("packer")
            .args(args.clone())
            .current_dir(&build_dir);

        let args_str = args.join(" ");
        info!("Running packer build command: packer {}", args_str);

        let mut build_state = PackerBuildState::default();

        let output = cmd
            .run_with_output_handler(|line| {
                // With -machine-readable flag, ALL output follows the format
                match parse_packer_event(&line.content) {
                    Ok(event) => {
                        log_packer_event(&event);
                        // Collect errors for build state tracking
                        if matches!(event, crate::parser::PackerEvent::UiMessage { ui_type, .. } if ui_type == "error") {
                            build_state.errors.push(line.content.clone());
                        }
                    }
                    Err(_) => {
                        // This shouldn't happen with -machine-readable, but just in case
                        eprintln!("Failed to parse packer output: {}", line.content);
                    }
                }
            })
            .await?;

        if output.success() {
            info!("Successfully built image: {}", config.name);

            if !build_state.artifacts.is_empty() {
                for artifact in &build_state.artifacts {
                    info!("Built artifact: {}", artifact);
                }
            } else {
                info!("Build completed successfully but no artifacts were created.");
            }
            Ok(())
        } else {
            let error_type = match output.exit_code {
                1 => "Usage or validation error",
                2 => "Error in configuration",
                3 => "Runtime error",
                _ => "Unknown error",
            };

            let duration_info = build_state
                .build_duration
                .map(|d| format!(" (build ran for {:?})", d))
                .unwrap_or_default();

            // Since errors are already displayed during the build process,
            // we don't need to include raw details in the final error message
            Err(Error::Packer(format!(
                "Packer build failed: {} (exit code {}){}",
                error_type, output.exit_code, duration_info
            )))
        }
    }

    fn find_template_file(&self, build_dir: &Path) -> Result<PathBuf> {
        let mut template_files = Vec::new();

        for entry in std::fs::read_dir(build_dir).map_err(|e| Error::Io(e))? {
            let entry = entry.map_err(|e| Error::Io(e))?;
            let path = entry.path();

            if path.is_file() && path.extension().and_then(|ext| ext.to_str()) == Some("hcl") {
                if let Some(file_name) = path.file_name().and_then(|f| f.to_str()) {
                    if !file_name.contains("pkrvars") && file_name != "packer_plugins.pkr.hcl" {
                        template_files.push(path);
                    }
                }
            }
        }

        if template_files.is_empty() {
            return Err(Error::Template(
                "No template file found in build directory".to_string(),
            ));
        }

        debug!("Found template files: {:?}", template_files);

        if template_files.len() > 1 {
            if let Some(template) = template_files.iter().find(|p| {
                p.file_stem()
                    .and_then(|s| s.to_str())
                    .map(|s| s.contains("base"))
                    .unwrap_or(false)
            }) {
                return Ok(template.clone());
            }

            Ok(template_files[0].clone())
        } else {
            Ok(template_files[0].clone())
        }
    }

    async fn prepare_build_dir(&self, config: &BuildConfig) -> Result<PathBuf> {
        let build_dir = if let Some(dir) = &config.working_dir {
            dir.clone()
        } else {
            let timestamp = chrono::Local::now().format("%Y%m%d%H%M%S");
            let build_dir = self
                .config
                .cache_dir
                .join("builds")
                .join(format!("{}-{}", config.name, timestamp));

            if !build_dir.exists() {
                fs::create_dir_all(&build_dir).await?;
            }

            build_dir
        };

        let template_path = &config.template_path;
        if !template_path.exists() {
            return Err(Error::Template(format!(
                "Template path not found: {:?}",
                template_path
            )));
        }
        debug!("Using template path: {:?}", template_path);

        let template_manager = TemplateManager::new();
        let template = template_manager.load(template_path.clone()).await?;

        debug!(
            "Template dependencies found: scripts={:?}, floppy={:?}, http={:?}, provisioners={:?}",
            template.dependencies.script_files,
            template.dependencies.floppy_files,
            template.dependencies.http_directories,
            template.dependencies.provisioner_files
        );

        let plugins_file = self
            .config
            .packer_dir
            .join("common")
            .join("packer_plugins.pkr.hcl");
        if plugins_file.exists() {
            let target = build_dir.join("packer_plugins.pkr.hcl");
            fs::copy(&plugins_file, &target).await?;
            debug!("Copied packer plugins file to build directory");
        }

        if template_path.is_file() {
            let file_name = template_path
                .file_name()
                .ok_or_else(|| Error::Template("Invalid template path".to_string()))?;
            let target = build_dir.join(file_name);
            fs::copy(template_path, &target).await?;
            debug!("Copied template file: {:?}", file_name);
        } else {
            let mut entries = fs::read_dir(template_path).await?;
            while let Some(entry) = entries.next_entry().await? {
                let path = entry.path();
                if path.is_file() && path.extension().and_then(|e| e.to_str()) == Some("hcl") {
                    if let Some(file_name) = path.file_name() {
                        let target = build_dir.join(file_name);
                        fs::copy(&path, &target).await?;
                        debug!("Copied template file: {:?}", file_name);
                    }
                }
            }
        }

        if template.dependencies.has_scripts() {
            self.copy_script_files(
                &template.dependencies.script_files,
                &config.platform,
                &build_dir,
            )
            .await?;
        }

        if template.dependencies.has_floppy() {
            self.copy_floppy_files(
                &template.dependencies.floppy_files,
                &config.platform,
                &build_dir,
            )
            .await?;
        }

        if template.dependencies.has_http() {
            self.copy_http_resources(&build_dir).await?;
        }

        if template.dependencies.has_provisioners() {
            self.copy_provisioner_files(
                &template.dependencies.provisioner_files,
                &config.platform,
                &build_dir,
            )
            .await?;
        }

        let template_parent = if template_path.is_file() {
            template_path
                .parent()
                .unwrap_or(Path::new(""))
                .to_path_buf()
        } else {
            template_path.clone()
        };

        for dir_name in &["scripts", "http", "files", "playbooks"] {
            let source_dir = template_parent.join(dir_name);
            if source_dir.exists() && source_dir.is_dir() {
                let target_dir = build_dir.join(dir_name);
                if !target_dir.exists() {
                    fs::create_dir_all(&target_dir).await?;
                }
                debug!("Copying template-specific resources from: {:?}", source_dir);
                copy_directory(&source_dir, &target_dir).await?;
            }
        }

        if !config.variables.is_empty() {
            let mut vars_content = String::new();
            for (key, value) in &config.variables {
                // Expand ~ in paths
                let expanded_value = if value.starts_with("~/") || value.starts_with("~\\") {
                    if let Ok(home) =
                        std::env::var("HOME").or_else(|_| std::env::var("USERPROFILE"))
                    {
                        value.replacen("~", &home, 1)
                    } else {
                        value.clone()
                    }
                } else {
                    value.clone()
                };

                let formatted_value =
                    if expanded_value.starts_with('"') && expanded_value.ends_with('"') {
                        expanded_value.clone()
                    } else if expanded_value == "true" || expanded_value == "false" {
                        expanded_value.clone()
                    } else if expanded_value.parse::<i64>().is_ok()
                        || expanded_value.parse::<f64>().is_ok()
                    {
                        expanded_value.clone()
                    } else {
                        format!("\"{}\"", expanded_value.replace('\"', "\\\""))
                    };

                vars_content.push_str(&format!("{} = {}\n", key, formatted_value));
            }

            fs::write(build_dir.join("variables.auto.pkrvars.hcl"), vars_content).await?;
            debug!("Wrote variables file to build directory");
        }

        Ok(build_dir)
    }

    async fn copy_script_files(
        &self,
        script_files: &HashSet<String>,
        platform: &Platform,
        build_dir: &Path,
    ) -> Result<()> {
        let script_dir = match platform {
            Platform::Windows => self
                .config
                .config_dir
                .join("infrastructure/scripts/windows"),
            Platform::Linux => self.config.config_dir.join("infrastructure/scripts/linux"),
        };

        if script_dir.exists() && script_dir.is_dir() {
            let target_dir = build_dir.join("scripts");
            if !target_dir.exists() {
                fs::create_dir_all(&target_dir).await?;
            }

            for script_name in script_files {
                let source_path = script_dir.join(script_name);
                if source_path.exists() {
                    let target_path = target_dir.join(script_name);
                    fs::copy(&source_path, &target_path).await?;
                    debug!("Copied script: {:?}", script_name);
                } else {
                    warn!("Referenced script not found: {:?}", source_path);
                }
            }
        } else {
            warn!("Script directory not found: {:?}", script_dir);
        }

        Ok(())
    }

    async fn copy_floppy_files(
        &self,
        floppy_files: &HashSet<String>,
        platform: &Platform,
        build_dir: &Path,
    ) -> Result<()> {
        let floppy_dir = match platform {
            Platform::Windows => self.config.config_dir.join("infrastructure/floppy/windows"),
            Platform::Linux => self.config.config_dir.join("infrastructure/floppy/linux"),
        };

        if floppy_dir.exists() && floppy_dir.is_dir() {
            let target_dir = build_dir.join("floppy");
            if !target_dir.exists() {
                fs::create_dir_all(&target_dir).await?;
            }

            for file_name in floppy_files {
                // First try the floppy directory
                let source_path = floppy_dir.join(file_name);
                if source_path.exists() {
                    let target_path = target_dir.join(file_name);
                    fs::copy(&source_path, &target_path).await?;
                    debug!("Copied floppy file from floppy dir: {:?}", file_name);
                } else {
                    // If not in floppy dir, check if it's a script file
                    let script_dir = match platform {
                        Platform::Windows => self
                            .config
                            .config_dir
                            .join("infrastructure/scripts/windows"),
                        Platform::Linux => {
                            self.config.config_dir.join("infrastructure/scripts/linux")
                        }
                    };
                    let script_path = script_dir.join(file_name);

                    if script_path.exists() {
                        // Copy the script to the build directory's scripts folder
                        let scripts_dir = build_dir.join("scripts");
                        if !scripts_dir.exists() {
                            fs::create_dir_all(&scripts_dir).await?;
                        }
                        let target_path = scripts_dir.join(file_name);
                        fs::copy(&script_path, &target_path).await?;
                        debug!("Copied floppy file from scripts dir: {:?}", file_name);
                    } else {
                        warn!(
                            "Referenced floppy file not found in floppy or scripts dir: {:?}",
                            file_name
                        );
                    }
                }
            }
        }

        Ok(())
    }

    async fn copy_http_resources(&self, build_dir: &Path) -> Result<()> {
        let http_dir = self.config.packer_dir.join("http");
        if http_dir.exists() && http_dir.is_dir() {
            let target_dir = build_dir.join("http");
            if !target_dir.exists() {
                fs::create_dir_all(&target_dir).await?;
            }
            copy_directory(&http_dir, &target_dir).await?;
            debug!("Copied HTTP directory");
        }
        Ok(())
    }

    async fn copy_provisioner_files(
        &self,
        provisioner_files: &HashSet<String>,
        platform: &Platform,
        build_dir: &Path,
    ) -> Result<()> {
        let playbooks_dir = match platform {
            Platform::Windows => self.config.packer_dir.join("playbooks/windows"),
            Platform::Linux => self.config.packer_dir.join("playbooks/linux"),
        };

        if playbooks_dir.exists() && playbooks_dir.is_dir() {
            let target_dir = build_dir.join("playbooks");
            if !target_dir.exists() {
                fs::create_dir_all(&target_dir).await?;
            }

            for playbook_name in provisioner_files {
                let source_path = playbooks_dir.join(playbook_name);
                if source_path.exists() {
                    let target_path = target_dir.join(playbook_name);
                    fs::copy(&source_path, &target_path).await?;
                    debug!("Copied playbook: {:?}", playbook_name);
                } else {
                    warn!("Referenced playbook not found: {:?}", source_path);
                }
            }
        }

        Ok(())
    }
}
