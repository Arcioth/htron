{
  description = "htron — Native Hyprland Desktop Suite (Ctron + Daeboard + Tunotron)";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

  outputs = { self, nixpkgs }:
    let
      systems = [ "x86_64-linux" "aarch64-linux" ];
      forAll = nixpkgs.lib.genAttrs systems;
    in {
      devShells = forAll (system:
        let pkgs = nixpkgs.legacyPackages.${system};
        in {
          default = pkgs.mkShell {
            nativeBuildInputs = with pkgs; [
              pkg-config
              gcc
              rustc
              cargo
            ];
            buildInputs = with pkgs; [
              gtk4
              libadwaita
              glib
              cairo
              pango
              gdk-pixbuf
              graphene
              mpv
            ];
          };
        });
    };
}
