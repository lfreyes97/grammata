#!/bin/sh
# Compila en release y deja dist/grammata-<versión>-linux-<arquitectura>.tar.gz
# (bin/, share/ e install.sh), con su suma SHA-256.
set -eu
cd "$(dirname "$0")/.."
ver="$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)"
name="grammata-$ver-linux-$(uname -m)"
cargo build --release --locked
rm -rf "dist/$name"
packaging/stage.sh "dist/$name" ""
install -m755 packaging/install.sh "dist/$name/install.sh"
tar -C dist -czf "dist/$name.tar.gz" "$name"
(cd dist && sha256sum "$name.tar.gz" > "$name.tar.gz.sha256")
echo "dist/$name.tar.gz"
