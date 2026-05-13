{
  description = "malbox - automated malware analysis sandbox";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };

    git-hooks = {
      url = "github:cachix/git-hooks.nix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs =
    {
      self,
      nixpkgs,
      rust-overlay,
      git-hooks,
    }:
    let
      system = "x86_64-linux";
      pkgs = import nixpkgs {
        inherit system;
        overlays = [ rust-overlay.overlays.default ];
        config.allowUnfree = true;
      };
      lib = pkgs.lib;

      # Windows cross-compilation packages
      # dontDisableStatic preserves the .a static archives needed by the linker.
      # See: https://discourse.nixos.org/t/statically-linked-mingw-binaries/38395/3
      mingwCrt = pkgs.pkgsCross.mingwW64.windows.mingw_w64.overrideAttrs {
        dontDisableStatic = true;
      };
      mingwPthreadsWin = pkgs.pkgsCross.mingwW64.windows.pthreads.overrideAttrs {
        dontDisableStatic = true;
      };

      # Rust's std for x86_64-pc-windows-gnu links -l:libpthread.a, but the Nix MinGW
      # toolchain uses MCF threads and doesn't ship winpthreads.
      mingwPthreads = pkgs.runCommand "mingw-libpthread" { } ''
        mkdir -p $out/lib
        ${pkgs.stdenv.cc.bintools}/bin/ar crs $out/lib/libpthread.a
      '';

      mingwMcfgthreads = pkgs.pkgsCross.mingwW64.windows.mcfgthreads;
      mingwCc = pkgs.pkgsCross.mingwW64.stdenv.cc;

      rustToolchain = pkgs.rust-bin.stable.latest.default.override {
        extensions = [
          "rust-src"
          "rust-analyzer"
          "clippy"
          "rustfmt"
        ];
        targets = [
          "x86_64-pc-windows-gnu"
          "x86_64-unknown-linux-gnu"
          "x86_64-unknown-linux-musl"
        ];
      };

      gitHooksCheck = git-hooks.lib.${system}.run {
        src = ./.;
        hooks = {
          cargo-check.enable = true;
          clippy.enable = true;
          rustfmt.enable = true;
        };
        settings = {
          rust.cargoManifestPath = "./back-end/Cargo.toml";
        };
      };

      # Per-directory package sets
      backendPackages =
        [ rustToolchain ]
        ++ (with pkgs; [
          # Database
          sqlx-cli
          postgresql_16
          process-compose

          # Build tools
          pkg-config
          cmake
          gcc
          flex
          bison
          dtc
          llvm_18
          clang_18
          protoc-gen-rust
          protobuf

          # Rust tools
          cargo-watch
          cargo-nextest
          git-cliff
          maturin

          # Languages
          python3
          perl

          # Python packages
          python3Packages.pywinrm
          python3Packages.pytest
          ruff

          # Infrastructure
          packer
          ansible

          # Utilities
          file

          # MinGW cross-compilation toolchain
          mingwCc.cc
          mingwCc.bintools
        ]);

      backendBuildInputs = with pkgs; [
        openssl
        libGL
        glib
        zlib
        pixman
        libllvm
        libvirt
        libclang.lib
      ];

      backendEnv = {
        LIBCLANG_PATH = "${pkgs.libclang.lib}/lib";
        BINDGEN_EXTRA_CLANG_ARGS = ''-I"${pkgs.glibc.dev}/include"'';
        LD_LIBRARY_PATH = lib.makeLibraryPath [
          pkgs.llvm_18
          pkgs.clang_18
          pkgs.libclang.lib
          pkgs.libvirt
        ];
        DATABASE_URL = "postgres://postgres@localhost:5432/malbox_db";
        CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER = "${mingwCc}/bin/${mingwCc.targetPrefix}gcc";
        CARGO_TARGET_X86_64_PC_WINDOWS_GNU_RUSTFLAGS = "-L native=${mingwCrt}/lib -L native=${mingwPthreadsWin}/lib -L native=${mingwPthreads}/lib -L native=${mingwMcfgthreads}/lib";
        MINGW_LIB_PATH = "${mingwCrt}/lib:${mingwMcfgthreads}/lib:${mingwPthreadsWin}/lib";
        MINGW_INCLUDE_PATH = "${mingwMcfgthreads.dev}/include";
      };

      frontendPackages = with pkgs; [
        nodejs
        pnpm
      ];

      docsPackages = with pkgs; [
        nodejs
      ];
    in
    {
      checks.${system} = {
        git-hooks = gitHooksCheck;
      };

      formatter.${system} = pkgs.nixfmt;

      devShells.${system} = {
        default = pkgs.mkShell (
          backendEnv
          // {
            inherit (gitHooksCheck) shellHook;
            packages = [ pkgs.nixd ] ++ backendPackages ++ frontendPackages ++ docsPackages;
            buildInputs = backendBuildInputs;
          }
        );

        backend = pkgs.mkShell (
          backendEnv
          // {
            inherit (gitHooksCheck) shellHook;
            packages = backendPackages;
            buildInputs = backendBuildInputs;
          }
        );

        frontend = pkgs.mkShell {
          packages = frontendPackages;
        };

        docs = pkgs.mkShell {
          packages = docsPackages;
        };
      };
    };
}
