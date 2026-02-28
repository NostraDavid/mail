{
  inputs = {
    nixpkgs.url = "github:cachix/devenv-nixpkgs/rolling";
    systems.url = "github:nix-systems/default";
    devenv.url = "github:cachix/devenv";
  };

  outputs = {
    self,
    nixpkgs,
    devenv,
    systems,
    ...
  }: let
    forEachSystem = nixpkgs.lib.genAttrs (import systems);
  in {
    devShells = forEachSystem (
      system: let
        pkgs = import nixpkgs {
          inherit system;
          overlays = [devenv.overlays.default];
        };
      in {
        default = devenv.lib.mkShell {
          inherit inputs pkgs;
          modules = [./devenv.nix];
        };
      }
    );
  };
}
