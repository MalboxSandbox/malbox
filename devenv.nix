{ pkgs, lib, config, inputs, ... }:

{
  languages = {
    rust = {
      enable = true;
      channel = "stable";
      components = [ "rustc" "cargo" "clippy" "rustfmt" "rust-analyzer" ];
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
