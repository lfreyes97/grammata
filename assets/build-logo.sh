#!/bin/sh
# Regenera el logo (SVG y PNG) desde logo.typ. Solo necesita el CLI de typst.
set -eu
cd "$(dirname "$0")"
T="typst compile --ignore-system-fonts"
mkdir -p icons
$T --format svg --input v=tile --input s=512 logo.typ logo.svg
$T --format svg --input v=negro --input s=512 logo.typ logo-negro.svg
$T --format svg --input v=blanco --input s=512 logo.typ logo-blanco.svg
$T --format svg --input v=horizontal-claro logo.typ logo-horizontal-claro.svg
$T --format svg --input v=horizontal-oscuro logo.typ logo-horizontal-oscuro.svg
for s in 16 24 32 48 64 128 256 512; do
  $T --format png --ppi 72 --input v=tile --input s=$s logo.typ icons/grammata-$s.png
done
