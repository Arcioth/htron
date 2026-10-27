{ pkgs ? import <nixpkgs> {} }:

pkgs.mkShell {
  name = "htron-dev";
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
}
