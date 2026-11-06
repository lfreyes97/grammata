#!/bin/sh
# Instala el lanzador y los iconos de Grammata para el usuario actual.
# El binario (`typst-editor`) debe estar en el PATH (p. ej. `cargo install --path .`).
set -eu
cd "$(dirname "$0")/.."
data="${XDG_DATA_HOME:-$HOME/.local/share}"
for s in 16 24 32 48 64 128 256 512; do
  install -Dm644 "assets/icons/grammata-$s.png" "$data/icons/hicolor/${s}x${s}/apps/grammata.png"
done
install -Dm644 assets/logo.svg "$data/icons/hicolor/scalable/apps/grammata.svg"
install -Dm644 packaging/grammata.desktop "$data/applications/grammata.desktop"
command -v update-desktop-database >/dev/null && update-desktop-database "$data/applications" || true
command -v gtk-update-icon-cache >/dev/null && gtk-update-icon-cache -q "$data/icons/hicolor" || true
echo "Instalado en $data"
