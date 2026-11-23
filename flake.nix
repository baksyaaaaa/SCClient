{
  description = "SCClient (SC SCLauncher) pour NixOS";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

  outputs = { self, nixpkgs }:
    let
      system = "x86_64-linux";
      pkgs = import nixpkgs {
        inherit system;
        config.allowUnfree = true;
      };

      scclient = pkgs.callPackage ./nix/package.nix { };
    in
    {
      packages.${system} = {
        inherit scclient;
        default = scclient;
      };

      apps.${system}.default = {
        type = "app";
        program = "${scclient}/bin/scclient";
      };

      overlays.default = final: prev: {
        scclient = final.callPackage ./nix/package.nix { };
      };
    };
}
