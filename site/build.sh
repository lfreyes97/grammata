#!/bin/sh
# Genera site/_out/ (HTML, CSS e imágenes) a partir de site/index.typ.
set -eu
cd "$(dirname "$0")"
rm -rf _out && mkdir -p _out/img
typst compile --features html --format html --root .. index.typ _out/index.html
# El tema por defecto es blueprint (temas/blueprint.css sobre style.css);
# `TEMA= ./build.sh` compila solo style.css.
TEMA="${TEMA-blueprint}"
cp style.css _out/
[ -z "${TEMA:-}" ] || cat "temas/$TEMA.css" >> _out/style.css
# Script del tema (vacío si no tiene).
if [ -n "${TEMA:-}" ] && [ -f "temas/$TEMA.js" ]; then cp "temas/$TEMA.js" _out/tema.js; else : > _out/tema.js; fi
cp ../assets/logo-horizontal-claro.svg ../assets/logo-horizontal-oscuro.svg \
   ../assets/icons/grammata-32.png img/ensayo-*.webp img/cartel-epigrafe-*.webp _out/img/
