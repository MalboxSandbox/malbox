{ pkgs, lib, config, inputs, ... }:

{
  languages = {
    javascript = {
      enable = true;
      npm.enable = true;
    };
  };

  # TODO: add mint CLI package here
}
