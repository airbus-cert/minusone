# source: https://github.com/nix-community/poetry2nix/blob/master/templates/app/flake.nix
{
  description = "tree-sitter-powershell";

  inputs = {
    flake-utils.url = "github:numtide/flake-utils";
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-25.05";
    fenix = {
      url = "github:nix-community/fenix/monthly";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = {
    self,
    nixpkgs,
    flake-utils,
    fenix,
  }:
    flake-utils.lib.eachDefaultSystem (system: let
      pkgs = nixpkgs.legacyPackages.${system};
    in {
      packages = {
        default = self.packages.${system}.myapp;
      };

      # Shell for app dependencies.
      #
      #     nix develop
      #
      # Use this shell for developing your app.
      devShells.default = pkgs.mkShell {
        packages = [
          pkgs.gnumake

          # Nix
          pkgs.nixpkgs-fmt
          pkgs.nil

          # Rust
          fenix.packages.${system}.default.toolchain

          # Python
          pkgs.python312
          pkgs.maturin
        ];
      };
    });
}
