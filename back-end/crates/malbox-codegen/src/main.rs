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
    std::fs::write(&cli.output, output)
        .map_err(|e| format!("failed to write {}: {e}", cli.output.display()))?;
    Ok(())
}

fn emit_cpp(r: &ResolvedRuntimeConfig) -> String {
    format!(
        "// AUTO-GENERATED from plugin.toml by malbox-codegen - do not edit by hand.\n\
         #pragma once\n\
         #include <cstddef>\n\
         #include <cstdint>\n\
         #include <malbox/runtime.hpp>\n\
         \n\
         namespace malbox::generated {{\n\
         inline constexpr ::malbox::RuntimeConfig runtime_config{{\n\
         \x20   .port                   = {port},\n\
         \x20   .sample_dir             = {sample_dir:?},\n\
         \x20   .artifact_dir           = {artifact_dir:?},\n\
         \x20   .stash_dir              = {stash_dir:?},\n\
         \x20   .log_dir                = {log_dir:?},\n\
         \x20   .stash_threshold_bytes  = {threshold},\n\
         \x20   .stash_ttl_secs         = {ttl},\n\
         \x20   .log_filter             = {log_filter:?},\n\
         }};\n\
         }} // namespace malbox::generated\n",
        port = r.port,
        sample_dir = r.sample_dir.to_string_lossy(),
        artifact_dir = r.artifact_dir.to_string_lossy(),
        stash_dir = r.stash_dir.to_string_lossy(),
        log_dir = r.log_dir.to_string_lossy(),
        threshold = r.stash_threshold_bytes,
        ttl = r.stash_ttl_secs,
        log_filter = r.log_filter,
    )
}
