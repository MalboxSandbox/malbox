{ pkgs, lib, config, inputs, ... }:

{
  languages.javascript = {
    enable = true;
    pnpm.enable = true;
  };
}
