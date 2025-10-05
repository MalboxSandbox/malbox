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
    clippy.enable = true;
    rustfmt.enable = true;
    cargo-check.enable = true;
  };
}
