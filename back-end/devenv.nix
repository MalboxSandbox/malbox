{ pkgs, lib, config, inputs, ... }:

let
  unstable = import inputs.nixpkgs-unstable { system = pkgs.stdenv.system; };
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
    python311Packages.sphinx
    python311Packages.sphinx-rtd-theme
    python311Packages.ninja
    llvm_18
    libllvm
    clang_18
    glibc
    packer
    terraform
    unstable.cocogitto
    protoc-gen-rust
    protobuf
    cargo-nextest
  ];
}
