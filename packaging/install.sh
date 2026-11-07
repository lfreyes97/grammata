#!/bin/sh
# Instalador del tar.gz: copia bin/ y share/ a PREFIX (por defecto ~/.local).
#   ./install.sh [PREFIX]
set -eu
cd "$(dirname "$0")"
prefix="${1:-$HOME/.local}"
mkdir -p "$prefix"
cp -r bin share "$prefix/"
command -v update-desktop-database >/dev/null && update-desktop-database "$prefix/share/applications" || true
# Solo si el prefijo ya es un tema de iconos (p. ej. ~/.local con hicolor propio).
[ -f "$prefix/share/icons/hicolor/index.theme" ] && command -v gtk-update-icon-cache >/dev/null && gtk-update-icon-cache -q "$prefix/share/icons/hicolor" || true
echo "Grammata instalado en $prefix (binario: $prefix/bin/grammata)"
