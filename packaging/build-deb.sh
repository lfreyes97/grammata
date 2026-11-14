#!/bin/sh
# Compila en release y deja dist/grammata_<versión>_<arquitectura>.deb
# Necesita dpkg-deb (Debian, Ubuntu o el runner de GitHub Actions).
#   SKIP_BUILD=1 packaging/build-deb.sh   usa el binario ya compilado
set -eu
cd "$(dirname "$0")/.."
command -v dpkg-deb >/dev/null || { echo "falta dpkg-deb" >&2; exit 1; }
ver="$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)"
arch="$(dpkg --print-architecture 2>/dev/null || echo amd64)"
name="grammata_${ver}_${arch}"
[ -n "${SKIP_BUILD:-}" ] || cargo build --release --locked
rm -rf "dist/$name"
packaging/stage.sh "dist/$name" /usr
mkdir -p "dist/$name/DEBIAN"
size="$(du -sk "dist/$name/usr" | cut -f1)"
cat > "dist/$name/DEBIAN/control" <<EOF
Package: grammata
Version: $ver
Section: editors
Priority: optional
Architecture: $arch
Installed-Size: $size
Maintainer: Luis Reyes <13804007+lfreyes97@users.noreply.github.com>
Depends: libc6, libgcc-s1, libfontconfig1, libxkbcommon0, libwayland-client0, libx11-6, libxcursor1, libxi6, libxrandr2, libvulkan1, libgl1
Recommends: mesa-vulkan-drivers
Suggests: pandoc
Homepage: https://github.com/lfreyes97/grammata
Description: Editor de Typst con vista previa del PDF real
 Grammata es un editor de escritorio nativo para Typst, escrito en Rust
 con egui: pestañas, paneles acoplables, autocompletado, plugins y vista
 previa del PDF real en vivo. El compilador de Typst va embebido.
EOF
dpkg-deb --root-owner-group --build "dist/$name" "dist/$name.deb"
(cd dist && sha256sum "$name.deb" > "$name.deb.sha256")
echo "dist/$name.deb"
