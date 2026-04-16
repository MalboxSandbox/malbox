mod dist_cpp_sdk;

use std::env;

fn main() {
    let args: Vec<String> = env::args().collect();
    let subcommand = args.get(1).map(String::as_str);

    match subcommand {
        Some("dist-cpp-sdk") => dist_cpp_sdk::run(),
        Some(other) => {
            eprintln!("error: unknown subcommand `{other}`");
            print_usage();
            std::process::exit(1);
        }
        None => {
            print_usage();
            std::process::exit(1);
        }
    }
}

fn print_usage() {
    eprintln!("usage: cargo xtask <subcommand> [options]");
    eprintln!();
    eprintln!("subcommands:");
    eprintln!("  dist-cpp-sdk [--target linux|windows]");
    eprintln!("      Build and package the C++ plugin SDK distribution.");
    eprintln!("      Default target: linux");
}
