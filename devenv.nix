{ pkgs, lib, config, inputs, ... }:

{
  languages = {
    rust = {
      enable = true;
      channel = "stable";
      components = [ "rustc" "cargo" "clippy" "rustfmt" "rust-analyzer" ];
      targets = [ "x86_64-pc-windows-gnu" "x86_64-unknown-linux-gnu" ];
    };
    python.enable = true;
    perl.enable = true;
  };

  git-hooks.hooks = {
    cargo-check.enable = true;
    clippy.enable = true;
    rustfmt.enable = true;
  };

  git-hooks.settings = {
    rust.cargoManifestPath = "./back-end/Cargo.toml";
  };
}
