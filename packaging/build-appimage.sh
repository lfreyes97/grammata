#!/bin/sh
# Deja dist/Grammata-<versión>-<arquitectura>.AppImage
# Necesita appimagetool (https://github.com/AppImage/appimagetool): en el
# PATH o en $APPIMAGETOOL. No empaqueta las bibliotecas del sistema
# (Wayland, X11, Vulkan, fontconfig): usa las del equipo.
#   SKIP_BUILD=1 packaging/build-appimage.sh   usa el binario ya compilado
set -eu
cd "$(dirname "$0")/.."
tool="${APPIMAGETOOL:-appimagetool}"
command -v "$tool" >/dev/null || { echo "falta appimagetool (o define APPIMAGETOOL)" >&2; exit 1; }
ver="$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)"
arch="$(uname -m)"
app="dist/Grammata.AppDir"
out="dist/Grammata-$ver-$arch.AppImage"
[ -n "${SKIP_BUILD:-}" ] || cargo build --release --locked
rm -rf "$app"
packaging/stage.sh "$app" /usr
ln -s usr/share/applications/grammata.desktop "$app/grammata.desktop"
ln -s usr/share/icons/hicolor/256x256/apps/grammata.png "$app/grammata.png"
cat > "$app/AppRun" <<'EOF'
#!/bin/sh
here="$(dirname "$(readlink -f "$0")")"
export XDG_DATA_DIRS="$here/usr/share:${XDG_DATA_DIRS:-/usr/local/share:/usr/share}"
exec "$here/usr/bin/grammata" "$@"
EOF
chmod +x "$app/AppRun"
ARCH="$arch" "$tool" "$app" "$out"
(cd dist && sha256sum "$(basename "$out")" > "$(basename "$out").sha256")
echo "$out"
