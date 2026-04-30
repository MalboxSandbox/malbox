use clap::{Parser, ValueEnum};
use malbox_plugin_manifest::{ResolvedRuntimeConfig, parse_manifest};
use std::path::PathBuf;
use std::process::ExitCode;

#[derive(Debug, Clone, ValueEnum)]
enum Lang {
    Cpp,
}

#[derive(Parser)]
#[command(
    version,
    about = "Emit compile-time runtime-config headers from plugin.toml"
)]
struct Cli {
    #[arg(long)]
    manifest: PathBuf,
    #[arg(long)]
    lang: Lang,
    #[arg(long)]
    output: PathBuf,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match run(cli) {
        Ok(()) => ExitCode::from(0),
        Err(e) => {
            eprintln!("malbox-codegen: {e}");
            ExitCode::from(1)
        }
    }
}

fn run(cli: Cli) -> Result<(), String> {
    let manifest = parse_manifest(&cli.manifest).map_err(|e| e.to_string())?;
    let resolved = ResolvedRuntimeConfig::from_raw(&manifest.runtime);
    resolved
        .validate(manifest.plugin.plugin_type)
        .map_err(|e| format!("invalid [runtime] in {}: {e}", cli.manifest.display()))?;

    let output = match cli.lang {
        Lang::Cpp => emit_cpp(&resolved),
    };
    if let Some(parent) = cli.output.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("failed to create {}: {e}", parent.display()))?;
    }
    std::fs::write(&cli.output, output)
        .map_err(|e| format!("failed to write {}: {e}", cli.output.display()))?;
    Ok(())
}

fn emit_cpp(r: &ResolvedRuntimeConfig) -> String {
    let ac_art_include = format_cpp_str_array(&r.auto_collect_artifacts.include);
    let ac_art_exclude = format_cpp_str_array(&r.auto_collect_artifacts.exclude);
    let ac_ext_include = format_cpp_str_array(&r.auto_collect_external_logs.include);
    let ac_ext_exclude = format_cpp_str_array(&r.auto_collect_external_logs.exclude);

    format!(
        "// AUTO-GENERATED from plugin.toml by malbox-codegen - do not edit by hand.\n\
         #pragma once\n\
         #include <cstddef>\n\
         #include <cstdint>\n\
         #include <malbox/runtime.hpp>\n\
         \n\
         namespace malbox::generated {{\n\
         \n\
         static constexpr const char* ac_art_include[] = {ac_art_include};\n\
         static constexpr const char* ac_art_exclude[] = {ac_art_exclude};\n\
         static constexpr const char* ac_ext_include[] = {ac_ext_include};\n\
         static constexpr const char* ac_ext_exclude[] = {ac_ext_exclude};\n\
         \n\
         inline constexpr ::malbox::RuntimeConfig runtime_config{{\n\
         \x20   .port                   = {port},\n\
         \x20   .sample_dir             = {sample_dir:?},\n\
         \x20   .artifact_dir           = {artifact_dir:?},\n\
         \x20   .stash_dir              = {stash_dir:?},\n\
         \x20   .log_dir                = {log_dir:?},\n\
         \x20   .external_log_dir       = {external_log_dir:?},\n\
         \x20   .stash_threshold_bytes  = {threshold},\n\
         \x20   .stash_ttl_secs         = {ttl},\n\
         \x20   .log_filter             = {log_filter:?},\n\
         \x20   .analysis_timeout       = {analysis_timeout},\n\
         \x20   .auto_collect_artifacts = {{\n\
         \x20       .enabled       = {ac_art_enabled},\n\
         \x20       .include       = ac_art_include,\n\
         \x20       .include_count = {ac_art_include_count},\n\
         \x20       .exclude       = ac_art_exclude,\n\
         \x20       .exclude_count = {ac_art_exclude_count},\n\
         \x20       .max_file_size = {ac_art_max},\n\
         \x20   }},\n\
         \x20   .auto_collect_external_logs = {{\n\
         \x20       .enabled       = {ac_ext_enabled},\n\
         \x20       .include       = ac_ext_include,\n\
         \x20       .include_count = {ac_ext_include_count},\n\
         \x20       .exclude       = ac_ext_exclude,\n\
         \x20       .exclude_count = {ac_ext_exclude_count},\n\
         \x20       .max_file_size = {ac_ext_max},\n\
         \x20   }},\n\
         }};\n\
         }} // namespace malbox::generated\n",
        port = r.port,
        sample_dir = r.sample_dir.to_string_lossy(),
        artifact_dir = r.artifact_dir.to_string_lossy(),
        stash_dir = r.stash_dir.to_string_lossy(),
        log_dir = r.log_dir.to_string_lossy(),
        external_log_dir = r.external_log_dir.to_string_lossy(),
        threshold = r.stash_threshold_bytes,
        ttl = r.stash_ttl_secs,
        log_filter = r.log_filter,
        analysis_timeout = r.analysis_timeout,
        ac_art_enabled = r.auto_collect_artifacts.enabled,
        ac_art_include_count = r.auto_collect_artifacts.include.len(),
        ac_art_exclude_count = r.auto_collect_artifacts.exclude.len(),
        ac_art_max = r.auto_collect_artifacts.max_file_size,
        ac_ext_enabled = r.auto_collect_external_logs.enabled,
        ac_ext_include_count = r.auto_collect_external_logs.include.len(),
        ac_ext_exclude_count = r.auto_collect_external_logs.exclude.len(),
        ac_ext_max = r.auto_collect_external_logs.max_file_size,
    )
}

fn format_cpp_str_array(patterns: &[String]) -> String {
    if patterns.is_empty() {
        return "{{}}".to_string();
    }
    let items: Vec<String> = patterns.iter().map(|p| format!("{p:?}")).collect();
    format!("{{{}}}", items.join(", "))
}
