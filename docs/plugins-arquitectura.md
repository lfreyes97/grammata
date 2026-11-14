# Plugins: funcionamiento y arquitectura

Este documento explica cómo están hechos los plugins por dentro. El formato
del `plugin.toml` y el protocolo, vistos por quien escribe un plugin, están
en [`plugins.md`](plugins.md).

## Idea general

Un plugin es una carpeta con un manifiesto. Aporta dos clases de cosas:

| Clase | Qué es | Dónde corre | Permiso |
|---|---|---|---|
| **Datos** | paquete typst, plantillas con formulario, fuentes, snippets, temas | el editor los lee | basta con tenerlo activo |
| **Programas** | importadores, exportadores, comandos | proceso aparte, por tuberías | el usuario lo permite a mano |

Nunca se carga código de un plugin dentro del proceso del editor. Hay tres
razones:

- **Licencia**: Grammata es GPL-3.0-or-later. Un plugin que solo son datos o
  un programa que habla por stdin/stdout no se enlaza con el editor, así que
  puede llevar otra licencia, también comercial.
- **Estabilidad**: un programa que falla o se cuelga no tumba el editor.
- **Simplicidad**: Rust no tiene ABI estable para bibliotecas dinámicas y
  WASM añadía una dependencia pesada.

## Archivos

| Archivo | Contenido |
|---|---|
| `src/plugins.rs` | manifiesto (`Manifest`, `Template`, `Field`), carga (`Plugin`), conjunto y estado guardado (`Plugins`, `Saved`), plugins de serie (`BUNDLED`) |
| `src/plugins/run.rs` | protocolo de programas externos: `Tool`, `Job`, `Request`, `Response` |
| `src/plugins/panel.rs` | panel «Plugins» y formularios de las plantillas |
| `src/plugins/ejemplo/`, `src/plugins/pandoc/` | plugins de serie, embebidos con `include_str!` |
| `src/engine.rs` | `set_extras`, `PackageDir`, `Files`: paquetes y fuentes de plugins en el compilador |
| `src/theme.rs` | `parse_theme`, `set_custom`: temas de plugins |
| `src/app.rs` | acciones de plugins, hilo del programa (`start_job`, `plugin_tick`) |
| `src/chrome.rs` | «Archivo → Importar…» y exportadores en el menú Exportar |
| `src/snippets.rs` | secciones «Plugin: nombre» en el panel Snippets |

En disco, del lado del usuario:

```
~/.config/grammata/
├── plugins/
│   └── <id>/
│       ├── plugin.toml        manifiesto
│       ├── typst.toml, …      paquete typst (opcional)
│       └── …                  fuentes, snippets.typ, temas, programas
└── plugins.json               desactivados, claves y permisos
```

## Ciclo de vida

```
App::new
  └─ Plugins::load()                  lee plugins.json
       └─ reload()                    una subcarpeta con plugin.toml = un plugin
            ├─ Plugin::load()         parsea el manifiesto (toml + serde)
            │    └─ resolve()         comprueba rutas, lee typst.toml, snippets y temas
            ├─ enabled / allowed      según plugins.json
            └─ publish()              engine::set_extras(paquetes, fuentes)
```

- **Carga**: `Plugins::load` se llama en `App::new` antes de `theme::apply`
  y de cualquier uso de las fuentes, porque los plugins aportan ambas cosas.
- **Errores**: un manifiesto que no parsea deja el plugin como `broken` (se
  lista con su error y no aporta nada). Los fallos parciales (una carpeta
  de fuentes que no existe, un tema mal escrito) se acumulan en
  `Plugin::errors` y el resto del plugin sigue funcionando. El manifiesto
  usa `deny_unknown_fields`: una clave mal escrita es un error visible.
- **Rutas**: todo lo que declara el manifiesto pasa por `inside()`, que
  rechaza rutas absolutas y `..`. No protege de enlaces simbólicos, que se
  permiten a propósito (un plugin puede enlazar a un paquete en desarrollo).
- **Estados**: `active()` = activado y no roto (aporta datos).
  `runnable()` = activo y con permiso (se ofrecen sus programas).
- **Cambios**: activar, desactivar, recargar o instalar pasa por una
  `Action`; `App::plugins_changed` recarga la lista de fuentes, fuerza una
  compilación y suelta el tema si su plugin ya no está. No se vigila la
  carpeta: tras editar un manifiesto hay que pulsar recargar.

`plugins.json` (`Saved`) guarda solo decisiones del usuario:

```json
{ "disabled": ["x"],
  "keys": { "x": "CLAVE" },
  "allowed": { "pandoc": ["pandoc -f markdown… -t typst…", "…"] } }
```

## Datos

### Paquete typst

El compilador resuelve los archivos con `engine::Files`, un `FileLoader`
que envuelve al de `typst-kit`:

```
#import "@local/tarjetas:0.1.0"
        │
        ▼
Files::load(id)
  ├─ ¿el paquete es de un plugin activo?  → se lee de su carpeta
  └─ si no                                → SystemFiles (sistema o Typst Universe)
```

- Los paquetes y las carpetas de fuentes de los plugins activos viven en un
  estado global del motor (`EXTRAS`), que `Plugins::publish` sustituye
  entero con `set_extras`. Cada compilación crea un `EngineWorld` nuevo y
  toma una copia (`Arc`), así que un cambio vale desde la siguiente.
- Nombre y versión salen del `typst.toml` del paquete; el `namespace`, del
  manifiesto (`local` por defecto).
- El paquete del plugin gana a uno instalado con el mismo nombre y versión.
- Fuera de Grammata el documento solo compila si el paquete está instalado.

### Fuentes

`shared_fonts()` construye el catálogo una vez: primero las carpetas de los
plugins, luego las del sistema y las de serie de typst. Si `set_extras`
recibe carpetas distintas, vacía el catálogo y se vuelve a escanear en el
siguiente uso. La barra de formato vuelve a pedir la lista de familias
(`FormatBar::reload_fonts`).

### Plantillas con formulario

Es el «frontend» de un paquete: el manifiesto declara los campos y un
cuerpo typst con marcadores.

```
[[template]] + [[template.field]]
        │   panel::form: un control por campo según `type`
        ▼
valores (texto por clave, en Plugins::forms)
        │   Template::render: {{clave}} → valor escrito en typst
        ▼
código typst ──► Action::NewDocWith  (documento nuevo sin guardar)
             └─► Action::InsertSnippet (en el cursor)
```

- Cada valor se guarda como texto y `Field::typst` lo escribe según el
  tipo: `content` → `[…]`, `string` → `"…"` con escapes, `color` →
  `rgb("#…")`, `number` → con su unidad, etc.
- `{{package}}` se sustituye por `@namespace/nombre:versión`, para no
  repetir la versión en cada plantilla.
- Un campo `optional` vacío elimina las líneas del cuerpo que lo usan (así
  desaparece el argumento entero).
- El formulario no tiene vista previa propia: la da la vista previa normal
  al crear el documento.

### Snippets y temas

- **Snippets**: cada archivo declarado es una `snippet_lib::Library`, la
  misma estructura del `snippets.typ` global, y se recarga sola si cambia.
- **Temas**: `theme::parse_theme` parte de la paleta clara u oscura de
  serie y sustituye los colores indicados. El tema elegido se guarda en
  `Prefs::theme` como `plugin/nombre` y se aplica con `theme::set_custom`,
  que hace que `palette_for` devuelva esa paleta. Alternar claro/oscuro
  quita el tema del plugin.

## Programas externos

### Modelo

Una **herramienta** (`run::Tool`) es una entrada `[[import]]`, `[[export]]`
o `[[command]]` del manifiesto con su línea de comando. No hay proceso
residente: cada uso lanza el programa, le da la entrada y espera la salida.

```
Acción (Importar, Exportar, botón de comando)
   │  App::import_path / App::plugin_run
   ▼
Plugins::job(...)  ──►  run::Job   (programa, texto, rutas, clave)
   │  App::start_job: hilo «plugin» + canal
   ▼
Job::run()                              hilo aparte
   ├─ lanza el programa (sin shell)
   ├─ escribe stdin, lee stdout y stderr a la vez
   └─ espera con límite de tiempo; si se pasa, lo mata
   │
   ▼  canal + request_repaint
App::plugin_tick  (cada frame)
   └─ según `Then`:  NewDoc │ Write(archivo) │ Wrote(archivo) │ Replace
```

- La interfaz no se bloquea: el resultado llega por un canal que
  `plugin_tick` consulta en cada frame. Solo hay un programa a la vez.
- El resultado no toca el documento directamente: `plugin_tick` emite
  acciones normales (`NewDocWith`, `ReplaceText`), que pasan por
  `App::run_actions` como cualquier otra edición.
- `Then::Replace` guarda el texto que se envió; si el documento cambió
  mientras tanto y ese texto ya no está, no se reemplaza nada.

### Protocolo (versión 1)

Común a los dos modos:

- stdin = entrada, stdout = resultado, en UTF-8.
- Código de salida distinto de 0 = error, y stderr es el mensaje.
- Directorio de trabajo: la carpeta del archivo, o la del plugin.
- Variables `GRAMMATA_PLUGIN_DIR`, `GRAMMATA_KIND`, `GRAMMATA_ID`,
  `GRAMMATA_FILE`.
- `{input}` y `{output}` en los argumentos pasan rutas, para formatos
  binarios: con `{input}` no se lee el archivo como texto y con `{output}`
  el archivo lo escribe el programa (`Then::Wrote`).

| `protocol` | stdin | stdout | Para qué |
|---|---|---|---|
| `text` (por defecto) | el texto | el texto | programas que ya existen (pandoc) |
| `json` | objeto `Request` | objeto `Response` | programas hechos para Grammata |

`Request` lleva `grammata` (versión del protocolo, hoy `1`), `editor`,
`kind`, `id`, `text`, `file`, `output`, `plugin_dir` y `license_key`.
`Response` lleva `text` y `message`, o `error`. La compatibilidad se
mantiene añadiendo campos: un programa debe ignorar los que no conozca, y
`Response` ignora los que no conoce el editor.

### Seguridad

Un plugin con programas ejecuta código con los permisos del usuario, así
que hay un permiso explícito y separado de «activo»:

- El panel muestra las líneas de comando y el botón **Permitir sus
  programas**. Hasta entonces `Plugins::tools()` no devuelve nada de ese
  plugin: no aparece en Importar, ni en Exportar, ni sus comandos.
- El permiso guarda las líneas autorizadas (`Saved::allowed`). Al recargar
  se comparan con las del manifiesto: si cambió algún comando, el permiso
  deja de valer.
- Los plugins de serie instalados con el botón + quedan permitidos: lo
  pidió el usuario y sus comandos vienen con el editor.
- No se usa shell: `run` es programa y argumentos, sin interpolación.
- `./programa` se resuelve dentro de la carpeta del plugin; lo demás, en el
  PATH.

Lo que el permiso **no** cubre: el programa no está aislado (no hay
sandbox) y el permiso se refiere a la línea de comando, no al contenido del
ejecutable. Si un `./programa` del plugin se sustituye por otro, el permiso
sigue valiendo.

### Licencias comerciales

- `needs_key = true` hace que el panel pida una clave, que se guarda en
  texto plano en `plugins.json`.
- El editor no la verifica: es software libre y cualquier comprobación
  interna se podría quitar.
- La clave se entrega en `license_key` a los programas con
  `protocol = "json"`, que son los únicos que pueden verificarla (el
  programa puede ser cerrado). Un plugin solo de datos no tiene dónde.

## Interfaz

- **Panel «Plugins»** (`Ctrl+Mayús+U`): por plugin, activar, versión,
  descripción, licencia, errores, clave, paquete (clic copia el `#import`),
  permiso de programas, temas, comandos y plantillas con su formulario. El
  panel solo cambia por su cuenta la clave y los valores del formulario; lo
  demás lo pide con acciones.
- **Acciones** (`src/actions.rs`): `PluginsReload`, `PluginEnable`,
  `PluginInstall`, `PluginAllow`, `SetTheme`, `NewDocWith`, `Import`,
  `ImportPath`, `PluginRun`.
- **Menús**: «Archivo → Importar…» elige el importador por la extensión del
  archivo (`Plugins::importer`). El menú Exportar lista los exportadores de
  los plugins con permiso.

## Pruebas

| Dónde | Qué cubre |
|---|---|
| `plugins.rs` | carga del ejemplo, relleno de plantillas, rutas fuera del plugin, manifiesto roto, desactivar, permiso que caduca, la plantilla compila contra el paquete del plugin, Markdown ida y vuelta con pandoc |
| `plugins/run.rs` (solo Unix) | texto y JSON, errores, programa inexistente, tiempo agotado, `{output}`, entradas grandes |
| `ui_tests.rs` (`--ignored`, con GPU) | formulario → documento que compila, tema, permiso, importar Markdown, comando |

Dos precauciones:

- `Plugins` publica en un estado global del motor. Las pruebas que crean
  uno toman el candado `ENGINE` para no pisarse.
- Las pruebas de UI fijan `XDG_CONFIG_HOME` a una carpeta temporal antes de
  crear la app. Sin eso escriben en la sesión real del usuario.

Las pruebas con pandoc se omiten si no está instalado. Los diálogos nativos
de archivo (Importar, Exportar de plugins) no están cubiertos.

## Cómo ampliar

- **Otro tipo de campo**: variante en `FieldKind`, su escritura en
  `Field::typst`, su valor inicial en `Field::initial` y su control en
  `panel::field`.
- **Otro formato con pandoc**: una entrada `[[import]]` o `[[export]]` en
  su manifiesto; no hace falta tocar el editor.
- **Otra contribución de datos**: campo en `Manifest`, carga en
  `Plugin::resolve` y consumo donde corresponda.
- **Otro plugin de serie**: carpeta en `src/plugins/` y entrada en
  `BUNDLED`.

## Límites y pendientes

- Sin proceso persistente ni eventos (guardar, compilar); sin cancelar un
  programa en marcha; un solo programa a la vez.
- Los comandos solo salen como botones en el panel (no hay menú ni paleta).
- Importar no se dispara al abrir o arrastrar un archivo.
- Los temas no cambian aún el fondo de los campos de texto ni el color de
  selección.
- No hay instalación desde `.zip` ni vigilancia de la carpeta.

La lista viva está en `TODO.md`, sección 9.
