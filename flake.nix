{
  description = "Grammata: editor de Typst con vista previa del PDF real";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

  outputs =
    { self, nixpkgs }:
    let
      systems = [
        "x86_64-linux"
        "aarch64-linux"
      ];
      forAll =
        f:
        nixpkgs.lib.genAttrs systems (
          system: f system nixpkgs.legacyPackages.${system}
        );
    in
    {
      packages = forAll (
        _: pkgs: rec {
          grammata = pkgs.callPackage ./nix/package.nix { };
          default = grammata;
        }
      );

      apps = forAll (
        system: _: {
          default = {
            type = "app";
            program = "${self.packages.${system}.default}/bin/grammata";
          };
        }
      );

      devShells = forAll (
        system: pkgs: {
          default = pkgs.mkShell {
            inputsFrom = [ self.packages.${system}.default ];
            packages = with pkgs; [
              rust-analyzer
              clippy
              rustfmt
              typst # para comparar y renderizar muestras
              imagemagick
            ];
            OPENSSL_NO_VENDOR = 1;
            LD_LIBRARY_PATH = pkgs.lib.makeLibraryPath self.packages.${system}.default.runtimeLibs;
          };
        }
      );

      overlays.default = final: _prev: {
        grammata = final.callPackage ./nix/package.nix { };
      };
    };
}
