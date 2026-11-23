{
  description = "CSClient (CS CSLauncher) pour NixOS";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

  outputs = { self, nixpkgs }:
    let
      system = "x86_64-linux";
      pkgs = import nixpkgs {
        inherit system;
        config.allowUnfree = true;
      };

      csclient = pkgs.callPackage ./nix/package.nix { };
    in
    {
      packages.${system} = {
        inherit csclient;
        default = csclient;
      };

      apps.${system}.default = {
        type = "app";
        program = "${csclient}/bin/csclient";
      };

      overlays.default = final: prev: {
        csclient = final.callPackage ./nix/package.nix { };
      };
    };
}
