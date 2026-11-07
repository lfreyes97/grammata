#!/bin/sh
# Instala en DESTDIR/PREFIX el árbol de Grammata: binario, iconos y lanzador.
# Lo usan el tar.gz, el PKGBUILD y los paquetes de CI.
#   packaging/stage.sh DESTDIR [PREFIX]      (PREFIX por defecto: /usr)
# El binario se toma de $BIN (por defecto target/release/grammata).
set -eu
cd "$(dirname "$0")/.."
dest="${1:?uso: stage.sh DESTDIR [PREFIX]}"
prefix="${2-/usr}"  # un PREFIX vacío es válido (raíz del tar.gz)
bin="${BIN:-target/release/grammata}"
root="$dest$prefix"
install -Dm755 "$bin" "$root/bin/grammata"
for s in 16 24 32 48 64 128 256 512; do
  install -Dm644 "assets/icons/grammata-$s.png" "$root/share/icons/hicolor/${s}x${s}/apps/grammata.png"
done
install -Dm644 assets/logo.svg "$root/share/icons/hicolor/scalable/apps/grammata.svg"
install -Dm644 packaging/grammata.desktop "$root/share/applications/grammata.desktop"
[ -f LICENSE ] && install -Dm644 LICENSE "$root/share/doc/grammata/LICENSE"
[ -f THIRD-PARTY-LICENSES.html ] && install -Dm644 THIRD-PARTY-LICENSES.html "$root/share/doc/grammata/THIRD-PARTY-LICENSES.html"
exit 0
