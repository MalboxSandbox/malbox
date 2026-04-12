{ pkgs, lib, config, inputs, ... }:

let
  unstable = import inputs.nixpkgs-unstable { system = pkgs.stdenv.system; };

  # Windows cross-compilation packages
  # dontDisableStatic preserves the .a static archives needed by the linker.
  # See: https://discourse.nixos.org/t/statically-linked-mingw-binaries/38395/3
  mingwCrt = pkgs.pkgsCross.mingwW64.windows.mingw_w64.overrideAttrs { dontDisableStatic = true; };
  mingwPthreadsWin = pkgs.pkgsCross.mingwW64.windows.pthreads.overrideAttrs { dontDisableStatic = true; };

  # Rust's std for x86_64-pc-windows-gnu links -l:libpthread.a, but the Nix MinGW
  # toolchain uses MCF threads and doesn't ship winpthreads.
  # Rust's std uses Windows APIs for threading directly.
  mingwPthreads = pkgs.runCommand "mingw-libpthread" {} ''
    mkdir -p $out/lib
    ${pkgs.stdenv.cc.bintools}/bin/ar crs $out/lib/libpthread.a
  '';

  mingwMcfgthreads = pkgs.pkgsCross.mingwW64.windows.mcfgthreads;
  mingwCc = pkgs.pkgsCross.mingwW64.stdenv.cc;
in {
  services.postgres = {
    enable = true;
    package = pkgs.postgresql_16;
    initialDatabases = [{
      name = "malbox_db";
    }];
    initialScript = ''
      CREATE ROLE postgres WITH LOGIN SUPERUSER PASSWORD 'password';
    '';
    settings = {
      listen_addresses = lib.mkForce "127.0.0.1";
      max_connections = 100;
    };
  };

  env = {
    LIBCLANG_PATH = "${pkgs.libclang.lib}/lib";
    BINDGEN_EXTRA_CLANG_ARGS = ''-I"${pkgs.glibc.dev}/include"'';
    LD_LIBRARY_PATH = lib.makeLibraryPath [ pkgs.llvm_18 pkgs.clang_18 pkgs.libclang.lib ];
    DATABASE_URL = "postgres://postgres@localhost:5432/malbox_db";
    CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER = "${mingwCc}/bin/${mingwCc.targetPrefix}gcc";
    CARGO_TARGET_X86_64_PC_WINDOWS_GNU_RUSTFLAGS = "-L native=${mingwCrt}/lib -L native=${mingwPthreadsWin}/lib -L native=${mingwPthreads}/lib -L native=${mingwMcfgthreads}/lib";

    # MinGW cross-compilation search paths for C++ (used by CMake toolchain).
    MINGW_LIB_PATH = "${mingwCrt}/lib:${mingwMcfgthreads}/lib:${mingwPthreadsWin}/lib";
    MINGW_INCLUDE_PATH = "${mingwMcfgthreads.dev}/include";
  };

  packages = with pkgs; [
    nixd
    sqlx-cli
    openssl
    cargo-watch
    file
    libGL
    glib
    flex
    bison
    dtc
    zlib
    pixman
    llvm_18
    libllvm
    libvirt
    clang_18
    packer
    ansible
    python3Packages.pywinrm
    unstable.cocogitto
    protoc-gen-rust
    protobuf
    cargo-nextest

    # C++ plugin SDK build dependencies
    cmake
    gcc

    # MinGW cross-compilation toolchain for Windows.
    # mingwCc.cc provides x86_64-w64-mingw32-g++ (unwrapped, no setup hooks).
    # mingwCc.bintools provides dlltool, ar, etc.
    mingwCc.cc
    mingwCc.bintools
  ];
}
