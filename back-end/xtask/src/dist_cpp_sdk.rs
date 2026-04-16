//! Build and package the C++ plugin SDK distribution.
//!
//! Produces a self-contained directory at `crates/malbox-plugin-sdk-cpp/dist/`
//! (or `dist-windows/` for Windows targets) containing the static library,
//! C/C++ headers, and CMake config files.

use std::{
    env,
    fs,
    path::{Path, PathBuf},
    process::Command,
};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Target {
    Linux,
    Windows,
}

impl Target {
    fn rust_target(self) -> &'static str {
        match self {
            Target::Linux => "x86_64-unknown-linux-gnu",
            Target::Windows => "x86_64-pc-windows-gnu",
        }
    }

    fn lib_name(self) -> &'static str {
        match self {
            Target::Linux => "libmalbox_plugin_sdk_cpp.a",
            Target::Windows => "malbox_plugin_sdk_cpp.lib",
        }
    }

    fn dist_dir_name(self) -> &'static str {
        match self {
            Target::Linux => "dist-linux",
            Target::Windows => "dist-windows",
        }
    }
}

pub fn run() {
    let target = parse_target();

    let root = workspace_root();
    let sdk_crate = root.join("crates/malbox-plugin-sdk-cpp");
    let dist = sdk_crate.join(target.dist_dir_name());

    // Clean previous dist
    let _ = fs::remove_dir_all(&dist);

    // Step 1: cargo build --release -p malbox-plugin-sdk-cpp [--target ...]
    let rust_target = target.rust_target();
    println!("Building malbox-plugin-sdk-cpp (release, {rust_target})...");

    let mut cargo_args = vec!["build", "--release", "-p", "malbox-plugin-sdk-cpp"];
    if target == Target::Windows {
        cargo_args.extend([
            "--target",
            rust_target,
            "--no-default-features",
            "--features",
            "guest",
        ]);
    }

    run_cmd(
        Command::new("cargo")
            .args(&cargo_args)
            .current_dir(&root),
    );

    // Step 2: Assemble dist/

    // dist/include/malbox_plugin.h
    let include_dst = dist.join("include");
    fs::create_dir_all(&include_dst)
        .unwrap_or_else(|e| panic!("failed to create `{}`: {e}", include_dst.display()));

    let header_src = sdk_crate.join("include/malbox_plugin.h");
    copy_file(&header_src, &include_dst.join("malbox_plugin.h"));

    // dist/include/malbox/*.hpp
    let malbox_hpp_dst = include_dst.join("malbox");
    copy_dir_glob(&sdk_crate.join("include/malbox"), "hpp", &malbox_hpp_dst);

    // dist/lib/<libname>
    let lib_dst = dist.join("lib");
    fs::create_dir_all(&lib_dst)
        .unwrap_or_else(|e| panic!("failed to create `{}`: {e}", lib_dst.display()));

    // Rust puts output under target/<triple>/release/ for cross builds,
    // and target/<triple>/release/ for native builds too (because .cargo/config.toml
    // sets [build] target = "x86_64-unknown-linux-gnu").
    let lib_src = root
        .join("target")
        .join(rust_target)
        .join("release")
        .join(target.lib_name());

    // For windows-gnu, cargo produces a .a file, not .lib
    let lib_src = if target == Target::Windows && !lib_src.exists() {
        root.join("target")
            .join(rust_target)
            .join("release")
            .join("libmalbox_plugin_sdk_cpp.a")
    } else {
        lib_src
    };

    copy_file(&lib_src, &lib_dst.join(lib_src.file_name().unwrap()));

    // dist/cmake/*.cmake
    let cmake_dst = dist.join("cmake");
    copy_dir_glob(&sdk_crate.join("cmake"), "cmake", &cmake_dst);

    // For Windows, also copy the MinGW toolchain file
    if target == Target::Windows {
        let toolchain_src = sdk_crate.join("cmake/mingw-w64-x86_64.cmake");
        if toolchain_src.exists() {
            copy_file(&toolchain_src, &cmake_dst.join("mingw-w64-x86_64.cmake"));
        }
    }

    println!("Done! Distribution assembled at: {}", dist.display());
}

fn parse_target() -> Target {
    let args: Vec<String> = env::args().collect();
    for (i, arg) in args.iter().enumerate() {
        if arg == "--target"
            && let Some(val) = args.get(i + 1)
        {
            return match val.as_str() {
                "windows" => Target::Windows,
                "linux" => Target::Linux,
                other => {
                    eprintln!("error: unknown target `{other}` (expected: linux, windows)");
                    std::process::exit(1);
                }
            };
        }
    }
    Target::Linux
}

fn workspace_root() -> PathBuf {
    // CARGO_MANIFEST_DIR is xtask/, so workspace root is one level up.
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    Path::new(manifest_dir)
        .parent()
        .expect("xtask manifest dir has no parent")
        .to_path_buf()
}

fn run_cmd(cmd: &mut Command) {
    let status = cmd
        .status()
        .unwrap_or_else(|e| panic!("failed to run `{cmd:?}`: {e}"));
    if !status.success() {
        eprintln!("command `{cmd:?}` failed with status {status}");
        std::process::exit(status.code().unwrap_or(1));
    }
}

fn copy_file(src: &Path, dst: &Path) {
    fs::copy(src, dst).unwrap_or_else(|e| {
        panic!(
            "failed to copy `{}` -> `{}`: {e}",
            src.display(),
            dst.display()
        )
    });
}

fn copy_dir_glob(src_dir: &Path, extension: &str, dst_dir: &Path) {
    fs::create_dir_all(dst_dir)
        .unwrap_or_else(|e| panic!("failed to create dir `{}`: {e}", dst_dir.display()));

    for entry in fs::read_dir(src_dir)
        .unwrap_or_else(|e| panic!("failed to read dir `{}`: {e}", src_dir.display()))
    {
        let entry = entry.expect("failed to read dir entry");
        let path = entry.path();
        if path.is_file()
            && let Some(ext) = path.extension()
            && ext == extension
        {
            copy_file(&path, &dst_dir.join(path.file_name().unwrap()));
        }
    }
}
