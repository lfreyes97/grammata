# Plugins de Grammata

Un plugin es una carpeta dentro de `~/.config/grammata/plugins/` con un
`plugin.toml`. No lleva código que se ejecute dentro del editor. Puede
aportar:

- un **paquete typst**, servido desde la carpeta (no hay que instalarlo);
- **plantillas con formulario**: el «frontend» del paquete;
- **fuentes**, **snippets** (`snippets.typ`) y **temas** del editor;
- **programas externos** que importan, exportan o transforman el texto.

Cómo está hecho por dentro: [`plugins-arquitectura.md`](plugins-arquitectura.md).

El panel **Plugins** (`Ctrl+Mayús+U`) los lista, los activa o desactiva y
con el botón **+** añade los de serie (Pandoc y uno de ejemplo para copiar). El botón de recargar vuelve a leer la carpeta tras editar un manifiesto.

## `plugin.toml`

```toml
id = "tarjetas"            # por defecto, el nombre de la carpeta
name = "Tarjetas"
version = "0.1.0"
description = "…"
authors = ["…"]
license = "MIT"            # texto libre: SPDX, «Comercial»…
needs_key = false          # true: el panel pide una clave de licencia
homepage = "https://…"

fonts = ["Fonts"]          # carpetas de fuentes
snippets = ["snippets.typ"]
themes = ["temas/sepia.toml"]

[package]                  # paquete typst del plugin
path = "."                 # carpeta con su typst.toml
namespace = "local"        # se importa como @local/<name>:<version>
```

Las rutas son relativas a la carpeta del plugin y no pueden salir de ella.

### Paquete

El nombre y la versión salen del `typst.toml` del paquete. Con el plugin
activo, `#import "@local/nombre:versión"` funciona en cualquier documento y
tiene prioridad sobre un paquete instalado con el mismo nombre. Fuera de
Grammata (CLI de `typst`) el documento solo compila si el paquete está
instalado en el sistema.

### Plantillas

```toml
[[template]]
id = "tarjeta"
name = "Tarjeta de cita"
group = "Redes"
description = "…"
body = """
#import "{{package}}": tarjeta
#tarjeta({{texto}}, autor: {{autor}}, fondo: {{fondo}})
"""

[[template.field]]
key = "texto"
label = "Texto"
type = "content"
default = "Todo es gracia."
```

Cada `{{clave}}` del cuerpo se sustituye por el valor del campo, ya escrito
en typst. `{{package}}` es `@namespace/nombre:versión`. El formulario ofrece
**Nuevo documento** e **Insertar** (en el cursor).

| `type` | Control | Se escribe como |
|---|---|---|
| `content` (por defecto) | texto | `[…]` |
| `string` | texto | `"…"` |
| `code` | texto monoespaciado | tal cual |
| `color` | selector | `rgb("#rrggbb")` |
| `number` | número (`unit`, `min`, `max`) | `72pt` |
| `bool` | casilla | `true` / `false` |
| `choice` | desplegable (`options`) | el valor de la opción, tal cual |
| `font` | familias instaladas | `"Familia"` |

Otros atributos de un campo: `hint`, `multiline = true` y `optional = true`
(si queda vacío, se quitan las líneas del cuerpo que lo usan). Las opciones
de `choice` son `"valor"` o `{ value = "…", label = "…" }`.

### Snippets

Igual que el `snippets.typ` global: cada `#let` público es un snippet y
aparecen en el panel Snippets bajo «Plugin: nombre».

### Temas

```toml
name = "Sepia"
dark = false          # de qué tema de serie hereda lo que no se indique

[colors]              # chrome surface surface_hover surface_active border
accent = "#9a5b13"    # border_strong text text_dim text_faint accent
                      # accent_soft on_accent ok warn err canvas
[syntax]              # comment punctuation escape strong link raw label
keyword = "#8a2f4f"   # heading list math keyword operator number string
                      # function interpolated
```

## Programas externos (protocolo 1)

Un plugin puede declarar programas que importan, exportan o transforman el
texto. Estos sí ejecutan código, en un proceso aparte: el panel muestra las
líneas de comando y no ofrece nada hasta que pulsas **Permitir sus
programas**. Si el manifiesto cambia un comando, hay que permitirlo otra vez.

```toml
[[import]]                 # Archivo → Importar…
id = "markdown"
name = "Markdown"
extensions = ["md", "markdown"]
run = ["pandoc", "--sandbox", "-f", "markdown-raw_attribute", "-t", "typst"]

[[export]]                 # menú Exportar
id = "markdown"
name = "Markdown"
extensions = ["md"]        # la primera es la que se escribe
run = ["pandoc", "-f", "typst", "-t", "commonmark_x"]

[[command]]                # botón en el panel Plugins
id = "mayus"
name = "A mayúsculas"
run = ["./mayus.py"]
input = "selection"        # o "document"; sin selección, el documento
output = "replace"         # o "new" (documento nuevo)
protocol = "json"          # por defecto "text"
timeout = 60               # segundos
```

`run` es el programa y sus argumentos (sin shell). `./x` es relativo a la
carpeta del plugin; lo demás se busca en el PATH. Cada uso lanza el programa
una vez:

- **Entrada** por stdin, **resultado** por stdout, en UTF-8.
- Código de salida distinto de 0 = error; stderr es el mensaje que se muestra.
- Directorio de trabajo: la carpeta del archivo (o la del plugin).
- Variables: `GRAMMATA_PLUGIN_DIR`, `GRAMMATA_KIND` (`import`, `export`,
  `command`), `GRAMMATA_ID` y `GRAMMATA_FILE` (si el documento tiene ruta).
- En los argumentos, `{input}` es la ruta del archivo importado y `{output}`
  la del exportado, para formatos binarios: con `{input}` no se manda nada
  por stdin y con `{output}` el archivo lo escribe el programa.

Con `protocol = "text"` stdin y stdout son el texto tal cual: sirve para
programas que ya existen. Con `protocol = "json"` son un objeto cada uno:

```json
{ "grammata": 1, "editor": "0.1.0", "kind": "import", "id": "markdown",
  "text": "…", "file": "/ruta/nota.md", "output": null,
  "plugin_dir": "/…/plugins/x", "license_key": "…" }
```

```json
{ "text": "…", "message": "aviso opcional para la barra de estado" }
{ "error": "por qué no se pudo" }
```

`grammata` es la versión del protocolo. Los campos nuevos se añadirán sin
cambiarla: un programa debe ignorar los que no conozca.

**Pandoc** viene de serie (botón + del panel): importa y exporta Markdown.
Su manifiesto trae comentado cómo añadir Word. La exportación pasa por el
lector de typst de pandoc, que no evalúa el documento: texto, títulos,
listas, tablas y ecuaciones pasan; funciones propias y paquetes, no.

## Licencias

Grammata es GPL-3.0-or-later. Un plugin son datos, código typst que el
editor lee y programas aparte que se comunican por tuberías, no código
enlazado con él, así que puede llevar la
licencia que quiera su autor, también comercial. (Esto describe la intención
del proyecto; no es asesoría legal.)

Con `needs_key = true` el panel pide una clave y la guarda en
`~/.config/grammata/plugins.json`. **El editor no la comprueba**: al ser
software libre, cualquier comprobación dentro de Grammata se podría quitar.
Se la pasa a los programas con `protocol = "json"` (`license_key`), que son
los que pueden verificarla. Un plugin solo de datos no tiene dónde hacerlo.
