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
    let raw = manifest.runtime.clone().unwrap_or_default();
    let resolved = ResolvedRuntimeConfig::from_raw(&raw);
    resolved
        .validate()
        .map_err(|e| format!("invalid [runtime] in {}: {e}", cli.manifest.display()))?;

    let output = match cli.lang {
        Lang::Cpp => emit_cpp(&resolved, raw.log_overflow_dir.is_none()),
    };
    std::fs::write(&cli.output, output)
        .map_err(|e| format!("failed to write {}: {e}", cli.output.display()))?;
    Ok(())
}

fn emit_cpp(r: &ResolvedRuntimeConfig, log_overflow_is_derived: bool) -> String {
    let log_overflow_literal = if log_overflow_is_derived {
        "nullptr".to_string()
    } else {
        format!("{:?}", r.log_overflow_dir.to_string_lossy())
    };
    let work_dir = r.work_dir.to_string_lossy().into_owned();
    format!(
        "// AUTO-GENERATED from plugin.toml by malbox-codegen — do not edit by hand.\n\
         #pragma once\n\
         #include <cstddef>\n\
         #include <cstdint>\n\
         #include <malbox/runtime.hpp>\n\
         \n\
         namespace malbox::generated {{\n\
         inline constexpr ::malbox::RuntimeConfig runtime_config{{\n\
         \x20   .port                   = {port},\n\
         \x20   .work_dir               = {work_dir:?},\n\
         \x20   .log_overflow_dir       = {log_overflow_literal},\n\
         \x20   .stash_threshold_bytes  = {threshold},\n\
         \x20   .stash_ttl_secs         = {ttl},\n\
         \x20   .log_filter             = {log_filter:?},\n\
         }};\n\
         }} // namespace malbox::generated\n",
        port = r.port,
        work_dir = work_dir,
        log_overflow_literal = log_overflow_literal,
        threshold = r.stash_threshold_bytes,
        ttl = r.stash_ttl_secs,
        log_filter = r.log_filter,
    )
}
