#!/bin/sh
# Genera las capturas de la web con la prueba `capturas_web` (egui_kittest, GPU).
#   site/capturas.sh [milisegundos_de_espera]
# Las capturas en bruto (PNG) van a site/capturas-raw/ (no se versiona) y las
# optimizadas (WebP, 1400 px de ancho) a site/img/, una por .typ de site/samples y tema.
# Un proceso por tema: la prueba no puede alternarlo sin corromper el editor.
set -eu
cd "$(dirname "$0")/.."
mkdir -p site/capturas-raw site/img
for tema in oscuro claro; do
  WEB_SAMPLES=site/samples SHOT_DIR="$PWD/site/capturas-raw" WEB_TEMA="$tema" WEB_WAIT_MS="${1:-3000}" \
    cargo test capturas_web -- --ignored
done
for png in site/capturas-raw/*.png; do
  magick "$png" -resize 1400x -quality 82 "site/img/$(basename "$png" .png).webp"
done
