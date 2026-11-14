<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="assets/logo-horizontal-oscuro.svg">
    <img alt="Grammata" src="assets/logo-horizontal-claro.svg" height="96">
  </picture>
</p>

# Grammata — editor de Typst en Rust + egui

Grammata es un editor de escritorio nativo para [Typst](https://typst.app), escrito en Rust con
egui: pestañas, paneles acoplables, autocompletado, plugins y vista previa del **PDF real** en vivo.

> **Estado:** probado en Linux. macOS no se ha probado y Windows solo se compilaría en CI,
> sin pruebas manuales.

## Uso

```bash
cd grammata
cargo run
```

Sin dependencias externas: el compilador typst 0.15.1 va embebido
(no hace falta el CLI en el PATH).

Atajos: `Ctrl+N` nuevo, `Ctrl+O` abrir (o arrastrar `.typ`), `Ctrl+S` / `Ctrl+Mayús+S`
guardar, `Ctrl+W` cerrar, `Ctrl+Tab` / `Ctrl+RePág/AvPág` / `Ctrl+1..9` cambiar de
pestaña, `F5` compilar, `Ctrl+E` exportar PDF, `Ctrl+F` buscar (`Enter`/`F3` siguiente,
`Mayús+Enter`/`Mayús+F3` anterior), `Ctrl+G` ir a línea, `Ctrl+Espacio` completar
(`Tab`/`Enter` acepta), `Ctrl+= / - / 0` fuente, `Alt+Z` ajuste de línea,
`Ctrl+Mayús+O / V / M` esquema · vista previa · problemas, `Ctrl+Mayús+Y / I / T / F / B` símbolos ·
snippets · plantillas · archivos · borrador, `Ctrl+rueda` zoom del PDF.

Sesión (pestañas, preferencias y diseño del dock) en
`~/.config/grammata/session.json`. Las sesiones del formato anterior se migran solas.

## Qué hace

- **Vista previa PDF**: el compilador embebido (`engine.rs`: crates `typst` /
  `typst-pdf` 0.15.1, sin CLI) compila el buffer vivo a PDF en un hilo aparte
  (`--root` = carpeta del gestor si contiene al documento, si no la del
  documento; así `#image`/`#include` relativos y paquetes `@preview` funcionan) y
  [`hayro`](https://crates.io/crates/hayro) lo rasteriza en otro hilo. Solo se
  pintan las páginas visibles (+1 pantalla), a la resolución exacta de zoom × densidad
  de pantalla; mientras llega el render nuevo se muestra el anterior escalado (sin
  parpadeo). Ajustar al ancho / página completa / zoom manual, navegación de páginas,
  `RePág/AvPág`. Exportar PDF copia el de la vista previa si está al día.
- **Paneles**: Esquema, Símbolos, Snippets, Vista previa y Problemas son pestañas de `egui_dock` (arrastrar,
  dividir, cerrar, reabrir desde la barra o el menú Ver). Diseños predefinidos: clásico,
  editor + vista previa, vista previa amplia, solo escritura.
- **Símbolos**: 136 símbolos (griegas, operadores, relaciones, conjuntos y lógica, flechas,
  otros) y 22 estructuras (`frac`, `sqrt`, `sum`, `mat`, `cases`…), con buscador y
  recientes. Fuera de una ecuación se insertan como `$…$` y el cursor queda dentro; dentro,
  se separan con espacios de los identificadores vecinos. Con texto seleccionado, las
  estructuras lo envuelven (`abs(sel)`).
- **Snippets**: biblioteca de serie (documento, texto, listas, figuras y tablas,
  matemáticas, cálculo con `calc`, código, diseño, referencias) y snippets propios guardados en
  `~/.config/grammata/snippets.json` (botón + crea uno desde la selección).
  Marcadores: `${sel}` selección · `${sel:x}` selección o `x` · `${x}` relleno que queda
  seleccionado para escribir encima · `${}` cursor.
- **Plugins**: carpetas en `~/.config/grammata/plugins/` con un `plugin.toml`: paquete typst con
  formulario, fuentes, snippets, temas y programas externos (importar y exportar Markdown con
  pandoc, que debe estar instalado). Los programas externos son código que se ejecuta en tu
  máquina: solo corren si los permites en el panel, y el permiso se pierde si cambia su línea de
  comando. El plugin de Pandoc usa `--sandbox` y no deja pasar typst, HTML ni TeX en crudo.
  Formato en [`docs/plugins.md`](docs/plugins.md); arquitectura en
  [`docs/plugins-arquitectura.md`](docs/plugins-arquitectura.md).
- **Plantillas**: builder de Artículo en el panel lateral (título, autor, fecha,
  idioma, papel, márgenes, fuente, numeración, índice, resumen) que abre el
  documento en una pestaña nueva; todo lo generado compila (`Ctrl+Mayús+T`).
- **Archivos**: gestor de una carpeta elegible (persistida) con árbol (guías de
  jerarquía, migas de pan, menú contextual), abrir, crear, renombrar y eliminar
  con confirmación; las pestañas abiertas siguen al renombrado (`Ctrl+Mayús+F`).
- **Borrador**: buffer temporal con vista previa en vivo para construir
  ecuaciones (toda la biblioteca de snippets + inserción rápida, modo `$…$`)
  o probar fragmentos y gráficos; se inserta en el cursor y persiste en la
  sesión (`Ctrl+Mayús+B`).
- **Problemas**: diagnósticos del compilador embebido con ubicación exacta
  (errores, advertencias, pistas); clic salta a la línea, también en archivos incluidos. Con errores se conserva
  la última vista previa válida y se avisa con un banner.
- **Editor**: widget propio (`code_view.rs`) con resaltado del analizador oficial
  `typst-syntax` (entiende marcado, código y matemáticas como el compilador), números de
  línea compatibles con el ajuste de línea y resalte de la línea actual; autocompletado
  de `#funciones` en un popup junto al cursor (el snippet deja seleccionado el hueco
  `..`); búsqueda con coincidencias resaltadas; cerrar con cambios pide confirmación.
- **Estilo**: paleta propia claro/oscuro (`theme.rs`), Inter + monoespaciada del sistema
  vía `fc-match`, iconos [Phosphor](https://phosphoricons.com).

## Marca

El logo (una Γ con cursor de texto) está hecho en Typst: `assets/logo.typ`.
`assets/build-logo.sh` regenera los SVG y los PNG de `assets/icons/`; solo usa
fuentes embebidas de typst. El nombre viene del griego γράμματα, «letras».

## Instalación

- **Linux (tar.gz):** `packaging/build-linux.sh` compila y genera
  `dist/grammata-<versión>-linux-<arquitectura>.tar.gz`; dentro, `./install.sh [PREFIX]`
  (por defecto `~/.local`) instala el binario `grammata`, los iconos y el lanzador.
- **Nix:** `nix run github:lfreyes97/grammata` o `nix profile install` (ver `flake.nix`).
- **Desde el código:** `cargo install --path .`

## Licencia

Grammata es software libre bajo la **GNU GPL v3 o posterior** (`LICENSE`).
Usa typst (Apache-2.0) y otras dependencias con licencias compatibles.
«Typst» es una marca de Typst GmbH; este proyecto es independiente y no está
afiliado a ella.

## Estructura

| Archivo | Qué contiene |
|---|---|
| `main.rs` | arranque de eframe |
| `app.rs` | estado (`State`), bucle por frame, atajos y ejecución de `Action`s, viewer del dock |
| `actions.rs` | `Action`: lo que pueden pedir menús, barras, atajos y paneles |
| `chrome.rs` | barra superior, barra de estado y diálogos |
| `editor.rs` | pestaña de documento, autocompletado, barra de búsqueda |
| `code_view.rs` | widget de edición: `TextEdit` + números de línea + resaltado cacheado |
| `highlight.rs` | resaltado y detección de ecuaciones con `typst-syntax` |
| `preview.rs` | lienzo PDF virtualizado, zoom, navegación |
| `pdf_render.rs` | hilo de rasterizado con `hayro` |
| `compile.rs` | hilo de compilación (buffer → PDF en proceso) |
| `diagnostics.rs` | diagnósticos del compilador (spans → línea/columna) |
| `engine.rs` | `World` typst embebido: archivos, paquetes, fuentes, PDF/PNG/SVG |
| `panels.rs` | paneles Esquema y Problemas |
| `symbols.rs` | panel de símbolos (tabla validada contra typst) |
| `plugins.rs` | plugins: manifiesto, plantillas con formulario, panel (`plugins/panel.rs`), programas externos (`plugins/run.rs`) |
| `snippets.rs` | panel de snippets, marcadores `${…}`, snippets del usuario |
| `templates.rs` | builders de plantillas (Artículo): opciones → `.typ` que compila |
| `files.rs` | gestor de archivos: árbol, abrir, crear, renombrar, eliminar |
| `scratch.rs` | borrador con vista previa en vivo e inserción en el documento |
| `layout.rs` | pestañas del dock, paneles, diseños predefinidos |
| `session.rs` | sesión persistente |
| `theme.rs` | paleta, estilos de egui/dock/editor, fuentes |
| `widgets.rs` | botones e iconos reutilizables |
| `doc.rs`, `text.rs`, `outline.rs`, `syntax.rs` | documentos, utilidades de texto, esquema, snippets del autocompletado |

## Pruebas

```bash
cargo test                      # unitarias
cargo test -- --include-ignored # + recorrido de la UI con egui_kittest (necesita typst y GPU)
                                #   y compilación de todos los símbolos, estructuras y snippets
SHOT_DIR=/tmp/capturas cargo test -- --include-ignored   # guarda capturas de cada paso
```

> Nota de fuentes: Inter y Adwaita Sans tienen glifos en el área de uso privado, la misma
> que Phosphor. `theme::icons_only` desactiva las letras de Phosphor en su `cmap` para
> poder ponerla delante sin que se coma el texto.
