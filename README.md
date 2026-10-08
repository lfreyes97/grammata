# Typst Editor — Rust + egui

Editor Typst con pestañas, paneles acoplables, autocompletado y vista previa del
**PDF real** en vivo.

## Uso

```bash
cd typst-editor
cargo run
```

Requiere `typst` CLI en PATH (probado con 0.15.1).

Atajos: `Ctrl+N` nuevo, `Ctrl+O` abrir (o arrastrar `.typ`), `Ctrl+S` / `Ctrl+Mayús+S`
guardar, `Ctrl+W` cerrar, `Ctrl+Tab` / `Ctrl+RePág/AvPág` / `Ctrl+1..9` cambiar de
pestaña, `F5` compilar, `Ctrl+E` exportar PDF, `Ctrl+F` buscar (`Enter`/`F3` siguiente,
`Mayús+Enter`/`Mayús+F3` anterior), `Ctrl+G` ir a línea, `Ctrl+Espacio` completar
(`Tab`/`Enter` acepta), `Ctrl+= / - / 0` fuente, `Alt+Z` ajuste de línea,
`Ctrl+Mayús+O / V / M` esquema · vista previa · problemas, `Ctrl+Mayús+Y / I` símbolos ·
snippets, `Ctrl+rueda` zoom del PDF.

Sesión (pestañas, preferencias y diseño del dock) en
`~/.config/typst-editor/session.json`. Las sesiones del formato anterior se migran solas.

## Qué hace

- **Vista previa PDF**: typst compila a PDF (texto por stdin, PDF por stdout,
  `--root` = carpeta del documento, así que `#image`/`#include` relativos funcionan) y
  [`hayro`](https://crates.io/crates/hayro) lo rasteriza en un hilo aparte. Solo se
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
  matemáticas, código, diseño, referencias) y snippets propios guardados en
  `~/.config/typst-editor/snippets.json` (botón + crea uno desde la selección).
  Marcadores: `${sel}` selección · `${sel:x}` selección o `x` · `${x}` relleno que queda
  seleccionado para escribir encima · `${}` cursor.
- **Problemas**: diagnósticos de `--diagnostic-format short` (errores, advertencias,
  pistas); clic salta a la línea, también en archivos incluidos. Con errores se conserva
  la última vista previa válida y se avisa con un banner.
- **Editor**: widget propio (`code_view.rs`) con resaltado del analizador oficial
  `typst-syntax` (entiende marcado, código y matemáticas como el compilador), números de
  línea compatibles con el ajuste de línea y resalte de la línea actual; autocompletado
  de `#funciones` en un popup junto al cursor (el snippet deja seleccionado el hueco
  `..`); búsqueda con coincidencias resaltadas; cerrar con cambios pide confirmación.
- **Estilo**: paleta propia claro/oscuro (`theme.rs`), Inter + monoespaciada del sistema
  vía `fc-match`, iconos [Phosphor](https://phosphoricons.com).

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
| `compile.rs` | hilo de compilación con la CLI de typst |
| `diagnostics.rs` | parser de diagnósticos de typst |
| `panels.rs` | paneles Esquema y Problemas |
| `symbols.rs` | panel de símbolos (tabla validada contra typst) |
| `snippets.rs` | panel de snippets, marcadores `${…}`, snippets del usuario |
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
